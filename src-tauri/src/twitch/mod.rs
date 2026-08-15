#[cfg(feature = "app")]
use crate::app_events::{
    emit_app_log, emit_twitch_auth_required, emit_twitch_chat_message, emit_twitch_status,
    AppLogLevel, TwitchAuthRequiredReason, TwitchStatus, TwitchStatusDomain,
};
#[cfg(feature = "app")]
use crate::settings::{default_twitch_client_id, AppState};
#[cfg(feature = "app")]
use crate::speech::enqueue_chat_message_for_speech;
use chrono::{DateTime, Utc};
#[cfg(feature = "app")]
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};
#[cfg(all(feature = "app", target_os = "linux"))]
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
};
#[cfg(feature = "app")]
use tauri::Manager;
#[cfg(feature = "app")]
use tokio_tungstenite::{connect_async, tungstenite::Message};

const TWITCH_DEVICE_URL: &str = "https://id.twitch.tv/oauth2/device";
const TWITCH_TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
const TWITCH_VALIDATE_URL: &str = "https://id.twitch.tv/oauth2/validate";
const TWITCH_USERS_URL: &str = "https://api.twitch.tv/helix/users";
#[cfg(feature = "app")]
const TWITCH_EVENTSUB_SUBSCRIPTIONS_URL: &str =
    "https://api.twitch.tv/helix/eventsub/subscriptions";
#[cfg(feature = "app")]
const TWITCH_EVENTSUB_WS_URL: &str = "wss://eventsub.wss.twitch.tv/ws?keepalive_timeout_seconds=30";
#[cfg(feature = "app")]
const EVENTSUB_BACKOFF_RESET_STABLE_DURATION: Duration = Duration::from_secs(30);
const CHAT_READ_SCOPE: &str = "user:read:chat";
const REQUIRED_TWITCH_SCOPES: &[&str] = &[CHAT_READ_SCOPE];
const KEYRING_SERVICE: &str = "rice.twitch.oauth";
const KEYRING_ACCOUNT: &str = "default";
const CHANNEL_CHAT_MESSAGE_TYPE: &str = "channel.chat.message";
const CHANNEL_CHAT_MESSAGE_VERSION: &str = "1";
const DEDUPE_CACHE_LIMIT: usize = 5_000;
const DEDUPE_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
#[cfg(all(feature = "app", target_os = "linux"))]
const LEGACY_AUTH_DIR: &str = ".rice";
#[cfg(all(feature = "app", target_os = "linux"))]
const LEGACY_AUTH_FILE: &str = "twitch-auth.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub platform: Platform,
    pub channel_id: String,
    pub channel_login: String,
    pub user_id: String,
    pub user_login: String,
    pub user_display_name: String,
    pub text: String,
    pub fragments: Vec<MessageFragment>,
    pub badges: Vec<ChatBadge>,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    Twitch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageFragment {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
    pub emote: Option<ChatEmote>,
    pub cheermote: Option<ChatCheermote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatEmote {
    pub id: String,
    #[serde(alias = "emote_set_id")]
    pub emote_set_id: String,
    #[serde(default)]
    #[serde(alias = "owner_id")]
    pub owner_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCheermote {
    pub prefix: String,
    pub bits: u32,
    pub tier: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatBadge {
    #[serde(alias = "set_id")]
    pub set_id: String,
    pub id: String,
    pub info: String,
}

#[cfg(feature = "app")]
#[derive(Debug)]
pub struct TwitchConnectionHandle {
    task: tokio::task::JoinHandle<()>,
}

#[cfg(feature = "app")]
impl TwitchConnectionHandle {
    fn new(task: tokio::task::JoinHandle<()>) -> Self {
        Self { task }
    }

    fn abort(&self) {
        self.task.abort();
    }
}

#[derive(Debug, Default, Clone)]
pub struct TwitchAuthState {
    generation: u64,
    pending: Option<PendingDeviceAuth>,
    token: Option<TwitchToken>,
    profile: Option<TwitchUserProfile>,
}

#[derive(Debug, Clone)]
struct PendingDeviceAuth {
    generation: u64,
    poll_in_flight: bool,
    client_id: String,
    device_code: String,
    interval: u64,
}

#[derive(Debug, Clone)]
struct TwitchToken {
    access_token: String,
    refresh_token: String,
    scopes: Vec<String>,
    expires_in: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTwitchAuth {
    client_id: String,
    access_token: String,
    refresh_token: String,
    scopes: Vec<String>,
    expires_in: u64,
    profile: TwitchUserProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchDeviceAuthStart {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub expires_at_ms: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchUserProfile {
    pub user_id: String,
    pub login: String,
    #[serde(default, skip_serializing)]
    pub client_id: String,
    pub scopes: Vec<String>,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum TwitchAuthPollResult {
    Pending {
        message: String,
        interval: u64,
    },
    SlowDown {
        message: String,
        interval: u64,
    },
    Authorized {
        profile: TwitchUserProfile,
        storage_warning: Option<String>,
    },
    Denied {
        message: String,
    },
    Expired {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchAuthValidationResult {
    pub profile: TwitchUserProfile,
    pub storage_warning: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    scope: Vec<String>,
    #[serde(default)]
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct ValidateResponse {
    client_id: String,
    login: String,
    user_id: String,
    scopes: Vec<String>,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct HelixUsersResponse {
    data: Vec<HelixUser>,
}

#[derive(Debug, Clone, Deserialize)]
struct HelixUser {
    id: String,
    login: String,
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct EventSubEnvelope {
    metadata: EventSubMetadata,
    #[serde(default)]
    payload: EventSubPayload,
}

#[derive(Debug, Deserialize)]
struct EventSubMetadata {
    message_id: String,
    message_type: String,
    #[serde(default)]
    message_timestamp: Option<String>,
    subscription_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct EventSubPayload {
    session: Option<EventSubSession>,
    subscription: Option<EventSubSubscription>,
    event: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct EventSubSession {
    id: String,
    keepalive_timeout_seconds: Option<u64>,
    reconnect_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EventSubSubscription {
    status: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct EventSubChatMessageEvent {
    broadcaster_user_id: String,
    broadcaster_user_login: String,
    chatter_user_id: String,
    chatter_user_login: String,
    chatter_user_name: String,
    message_id: String,
    message: EventSubChatMessageBody,
    #[serde(default)]
    badges: Vec<ChatBadge>,
}

#[derive(Debug, Deserialize)]
struct EventSubChatMessageBody {
    text: String,
    #[serde(default)]
    fragments: Vec<MessageFragment>,
}

#[derive(Debug, Deserialize)]
struct OAuthErrorResponse {
    message: Option<String>,
    error: Option<String>,
}

#[cfg(feature = "app")]
#[derive(Debug, Clone)]
struct EventSubConnectionParams {
    broadcaster_user_id: String,
    broadcaster_login: String,
    user_id: String,
}

#[cfg(feature = "app")]
#[derive(Debug, Clone)]
struct EventSubAuthCredentials {
    client_id: String,
    access_token: String,
    refresh_token: String,
}

#[cfg(feature = "app")]
#[derive(Debug, thiserror::Error)]
enum SubscriptionRequestError {
    #[error("Twitch EventSub 購読の認証が拒否されました。")]
    Unauthorized,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[cfg(feature = "app")]
enum EventSubSessionExit {
    Reconnect(String),
}

#[cfg(feature = "app")]
#[derive(Debug, Default)]
struct EventSubReconnectBackoff {
    failed_attempts: u64,
    established_at: Option<Instant>,
}

#[cfg(feature = "app")]
impl EventSubReconnectBackoff {
    fn record_session_established(&mut self) {
        self.record_session_established_at(Instant::now());
    }

    fn record_session_established_at(&mut self, established_at: Instant) {
        self.established_at = Some(established_at);
    }

    fn record_handover_started(&mut self) {
        self.established_at = None;
    }

    fn next_delay_after_failure(&mut self) -> u64 {
        self.next_delay_after_failure_at(Instant::now())
    }

    fn next_delay_after_failure_at(&mut self, failed_at: Instant) -> u64 {
        if self.established_at.is_some_and(|established_at| {
            failed_at.saturating_duration_since(established_at)
                >= EVENTSUB_BACKOFF_RESET_STABLE_DURATION
        }) {
            self.failed_attempts = 0;
        }

        self.established_at = None;
        self.failed_attempts = self.failed_attempts.saturating_add(1);
        retry_backoff_seconds(self.failed_attempts)
    }
}

impl From<ValidateResponse> for TwitchUserProfile {
    fn from(value: ValidateResponse) -> Self {
        Self {
            user_id: value.user_id,
            login: value.login,
            client_id: value.client_id,
            scopes: value.scopes,
            expires_in: value.expires_in,
        }
    }
}

impl TwitchAuthState {
    fn invalidate_operations(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.pending = None;
        self.generation
    }

    fn pending_is_current(&self, generation: u64) -> bool {
        self.generation == generation && self.pending.is_some()
    }
    fn profile(&self) -> Option<TwitchUserProfile> {
        self.profile.clone()
    }

    fn restore(stored: StoredTwitchAuth) -> anyhow::Result<Self> {
        let mut profile = stored.profile;
        if profile.client_id.trim().is_empty() {
            profile.client_id = stored.client_id.clone();
        }
        let scopes = token_scopes(stored.scopes, &profile);
        ensure_required_twitch_scopes(&scopes)?;
        Ok(Self {
            generation: 0,
            pending: None,
            token: Some(TwitchToken {
                access_token: stored.access_token,
                refresh_token: stored.refresh_token,
                scopes,
                expires_in: stored.expires_in,
            }),
            profile: Some(profile),
        })
    }

    fn stored_auth(&self) -> Option<StoredTwitchAuth> {
        let token = self.token.as_ref()?;
        let profile = self.profile.clone()?;
        Some(StoredTwitchAuth {
            client_id: profile.client_id.clone(),
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
            scopes: token.scopes.clone(),
            expires_in: token.expires_in,
            profile,
        })
    }

    #[cfg(feature = "app")]
    fn eventsub_credentials(&self) -> anyhow::Result<EventSubAuthCredentials> {
        let token = self
            .token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Twitch にログインしていません。"))?;
        let profile = self.profile.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Twitch のユーザー情報がありません。認証を確認してください。")
        })?;
        let client_id = if profile.client_id.trim().is_empty() {
            default_twitch_client_id()
        } else {
            profile.client_id.clone()
        };

        if client_id.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "Twitch Client ID が見つかりません。再ログインしてください。"
            ));
        }

        ensure_required_twitch_scopes(&profile.scopes)?;

        Ok(EventSubAuthCredentials {
            client_id,
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
        })
    }

    fn replace_token(
        &mut self,
        token: TokenResponse,
        profile: TwitchUserProfile,
    ) -> anyhow::Result<String> {
        // A refresh response can omit `scope`.  The profile obtained by validating the
        // newly-issued access token is therefore the authoritative source here; do
        // not retain the scopes of the token that was just rejected by EventSub.
        ensure_required_twitch_scopes(&profile.scopes)?;
        let scopes = token_scopes(token.scope, &profile);
        let access_token = token.access_token.clone();
        self.token = Some(TwitchToken {
            access_token,
            refresh_token: token.refresh_token,
            scopes,
            expires_in: token.expires_in,
        });
        self.profile = Some(profile);
        Ok(token.access_token)
    }
}

#[cfg(feature = "app")]
pub struct TwitchAuthStore;

#[cfg(feature = "app")]
trait AuthSecretStore {
    fn load_secret(&self) -> anyhow::Result<Option<String>>;
    fn save_secret(&self, secret: &str) -> anyhow::Result<()>;
    fn clear_secret(&self) -> anyhow::Result<()>;
}

#[cfg(feature = "app")]
struct AuthStorage<'a, SecureStore, LegacyStore> {
    secure: &'a SecureStore,
    legacy: &'a LegacyStore,
}

#[cfg(feature = "app")]
pub(crate) struct AuthLoadResult {
    pub(crate) auth: Option<TwitchAuthState>,
    pub(crate) storage_warning: Option<String>,
}

#[cfg(feature = "app")]
impl<SecureStore: AuthSecretStore, LegacyStore: AuthSecretStore>
    AuthStorage<'_, SecureStore, LegacyStore>
{
    fn load(&self) -> AuthLoadResult {
        match self.secure.load_secret() {
            Ok(Some(secret)) => match restore_stored_auth(&secret) {
                Ok(auth) => AuthLoadResult {
                    auth: Some(auth),
                    storage_warning: self
                        .legacy
                        .clear_secret()
                        .err()
                        .map(to_legacy_cleanup_user_message),
                },
                Err(error) => AuthLoadResult {
                    auth: None,
                    storage_warning: Some(format!(
                        "OS の資格情報ストアにある Twitch 認証情報を読み込めませんでした。Login から再認証してください: {error}"
                    )),
                },
            },
            Ok(None) => self.migrate_legacy_auth(None),
            Err(error) => self.migrate_legacy_auth(Some(error)),
        }
    }

    fn migrate_legacy_auth(&self, secure_load_error: Option<anyhow::Error>) -> AuthLoadResult {
        let secret = match self.legacy.load_secret() {
            Ok(Some(secret)) => secret,
            Ok(None) => {
                return AuthLoadResult {
                    auth: None,
                    storage_warning: secure_load_error.map(to_secure_store_load_user_message),
                }
            }
            Err(error) => {
                return AuthLoadResult {
                    auth: None,
                    storage_warning: Some(to_auth_recovery_failure_user_message(
                        secure_load_error,
                        error,
                    )),
                }
            }
        };

        let auth = match restore_stored_auth(&secret) {
            Ok(auth) => auth,
            Err(error) => {
                return AuthLoadResult {
                    auth: None,
                    storage_warning: Some(to_auth_recovery_failure_user_message(
                        secure_load_error,
                        error,
                    )),
                }
            }
        };

        match self.secure.save_secret(&secret) {
            Ok(()) => AuthLoadResult {
                auth: Some(auth),
                storage_warning: self.legacy.clear_secret().err().map_or_else(
                    || {
                        Some(
                            "以前のローカル認証情報を OS の資格情報ストアへ移行し、平文ファイルを削除しました。"
                                .to_string(),
                        )
                    },
                    |error| {
                        Some(format!(
                            "以前のローカル認証情報を OS の資格情報ストアへ移行しましたが、平文ファイルを削除できませんでした。{}",
                            to_legacy_cleanup_user_message(error)
                        ))
                    },
                ),
            },
            Err(error) => AuthLoadResult {
                auth: None,
                storage_warning: Some(to_auth_recovery_failure_user_message(
                    secure_load_error,
                    error,
                )),
            },
        }
    }

    fn save(&self, auth: &TwitchAuthState) -> anyhow::Result<Option<String>> {
        let stored = auth
            .stored_auth()
            .ok_or_else(|| anyhow::anyhow!("保存できる Twitch 認証状態がありません。"))?;
        let secret = serde_json::to_string(&stored)?;

        match self.secure.save_secret(&secret) {
            Ok(()) => Ok(self
                .legacy
                .clear_secret()
                .err()
                .map(to_legacy_cleanup_user_message)),
            Err(error) => Ok(Some(to_session_only_user_message(error))),
        }
    }

    fn clear(&self) -> anyhow::Result<()> {
        let secure_result = self.secure.clear_secret();
        let legacy_result = self.legacy.clear_secret();
        match (secure_result, legacy_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Err(secure_error), Err(legacy_error)) => {
                Err(anyhow::anyhow!("{secure_error}; {legacy_error}"))
            }
        }
    }
}

#[cfg(feature = "app")]
impl TwitchAuthStore {
    pub(crate) fn load() -> AuthLoadResult {
        AuthStorage {
            secure: &KeyringAuthStore,
            legacy: &LegacyAuthStore,
        }
        .load()
    }

    fn save(auth: &TwitchAuthState) -> anyhow::Result<Option<String>> {
        AuthStorage {
            secure: &KeyringAuthStore,
            legacy: &LegacyAuthStore,
        }
        .save(auth)
    }

    fn clear() -> anyhow::Result<()> {
        AuthStorage {
            secure: &KeyringAuthStore,
            legacy: &LegacyAuthStore,
        }
        .clear()
    }
}

#[cfg(feature = "app")]
struct KeyringAuthStore;

#[cfg(feature = "app")]
impl AuthSecretStore for KeyringAuthStore {
    fn load_secret(&self) -> anyhow::Result<Option<String>> {
        let entry = keyring_entry()?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn save_secret(&self, secret: &str) -> anyhow::Result<()> {
        keyring_entry()?.set_password(secret)?;
        Ok(())
    }

    fn clear_secret(&self) -> anyhow::Result<()> {
        match keyring_entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(feature = "app")]
fn keyring_entry() -> anyhow::Result<keyring::Entry> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)?)
}

#[cfg(feature = "app")]
struct LegacyAuthStore;

#[cfg(all(feature = "app", target_os = "linux"))]
impl AuthSecretStore for LegacyAuthStore {
    fn load_secret(&self) -> anyhow::Result<Option<String>> {
        load_legacy_auth_secret()
    }

    fn save_secret(&self, _secret: &str) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("平文の認証情報ファイルは作成しません。"))
    }

    fn clear_secret(&self) -> anyhow::Result<()> {
        clear_legacy_auth()
    }
}

#[cfg(all(feature = "app", not(target_os = "linux")))]
impl AuthSecretStore for LegacyAuthStore {
    fn load_secret(&self) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    fn save_secret(&self, _secret: &str) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("平文の認証情報ファイルは作成しません。"))
    }

    fn clear_secret(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "app")]
fn restore_stored_auth(secret: &str) -> anyhow::Result<TwitchAuthState> {
    serde_json::from_str::<StoredTwitchAuth>(secret)
        .map(TwitchAuthState::restore)
        .map_err(anyhow::Error::from)?
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn load_legacy_auth_secret() -> anyhow::Result<Option<String>> {
    let path = match legacy_auth_path() {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };
    if !path.exists() {
        return Ok(None);
    }

    ensure_legacy_permissions(&path)?;
    Ok(Some(fs::read_to_string(path)?))
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn clear_legacy_auth() -> anyhow::Result<()> {
    let path = match legacy_auth_path() {
        Ok(path) => path,
        Err(_) => return Ok(()),
    };
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn legacy_auth_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        anyhow::anyhow!("HOME が設定されていないため、Twitch 認証情報を保存できません。")
    })?;
    Ok(PathBuf::from(home)
        .join(LEGACY_AUTH_DIR)
        .join(LEGACY_AUTH_FILE))
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn ensure_legacy_parent_permissions(path: &Path) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Twitch 認証情報の保存先ディレクトリが見つかりません。"))?;
    if parent.exists() {
        if !parent.is_dir() {
            return Err(anyhow::anyhow!(
                "Twitch 認証情報の保存先がディレクトリではありません: {}",
                parent.display()
            ));
        }
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        return Ok(());
    }

    fs::DirBuilder::new().mode(0o700).create(parent)?;
    Ok(())
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn ensure_legacy_permissions(path: &Path) -> anyhow::Result<()> {
    ensure_legacy_parent_permissions(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[allow(dead_code)]
pub trait TwitchChatSource {
    fn connect(
        &self,
        channel: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
    fn disconnect(&self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn twitch_start_auth(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<TwitchDeviceAuthStart, String> {
    let client_id = default_twitch_client_id();

    if client_id.is_empty() {
        return Err("Twitch Client ID がビルド設定にありません。RICE_TWITCH_CLIENT_ID を設定してビルドしてください。".to_string());
    }

    let generation = {
        let mut auth = state
            .twitch_auth
            .lock()
            .map_err(|error| error.to_string())?;
        auth.invalidate_operations()
    };

    let response = request_device_code(&client_id)
        .await
        .map_err(to_twitch_user_message)?;
    let auth_start = TwitchDeviceAuthStart {
        user_code: response.user_code.clone(),
        verification_uri: response.verification_uri,
        expires_in: response.expires_in,
        expires_at_ms: std::time::SystemTime::now()
            .checked_add(std::time::Duration::from_secs(response.expires_in))
            .and_then(|deadline| deadline.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|deadline| deadline.as_millis() as u64)
            .unwrap_or(u64::MAX),
        interval: response.interval,
    };

    let mut auth = state
        .twitch_auth
        .lock()
        .map_err(|error| error.to_string())?;
    if auth.generation != generation {
        return Err(
            "新しい Twitch 認証操作が開始されたため、古い認証コードを破棄しました。".to_string(),
        );
    }
    auth.token = None;
    auth.profile = None;
    auth.pending = Some(PendingDeviceAuth {
        generation,
        poll_in_flight: false,
        client_id,
        device_code: response.device_code,
        interval: response.interval,
    });
    emit_twitch_status(
        &app,
        TwitchStatusDomain::Auth,
        TwitchStatus::AuthRequired,
        Some("Twitch 認証コードを発行しました。".to_string()),
    );
    emit_app_log(&app, AppLogLevel::Info, "Twitch 認証コードを発行しました。");

    Ok(auth_start)
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn twitch_poll_auth(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<TwitchAuthPollResult, String> {
    let pending = {
        let mut auth = state
            .twitch_auth
            .lock()
            .map_err(|error| error.to_string())?;
        let pending = auth
            .pending
            .as_mut()
            .ok_or_else(|| "Twitch 認証が開始されていません。".to_string())?;
        if pending.poll_in_flight {
            return Err("Twitch 認証を確認中です。完了までお待ちください。".to_string());
        }
        pending.poll_in_flight = true;
        pending.clone()
    };

    match poll_device_token(&pending).await {
        Ok(token) => {
            ensure_pending_auth_is_current(&state, pending.generation)?;
            let profile = match validate_access_token(&token.access_token).await {
                Ok(profile) => profile,
                Err(error) => {
                    clear_poll_in_flight_if_current(&state, pending.generation)?;
                    return Err(to_twitch_user_message(error));
                }
            };
            let profile = TwitchUserProfile::from(profile);
            if let Err(error) = ensure_required_twitch_scopes(&profile.scopes) {
                let message = error.to_string();
                {
                    let mut auth = state
                        .twitch_auth
                        .lock()
                        .map_err(|error| error.to_string())?;
                    if auth.generation != pending.generation {
                        return Err(
                            "新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。"
                                .to_string(),
                        );
                    }
                    auth.pending = None;
                    auth.token = None;
                    auth.profile = None;
                }
                emit_twitch_auth_required(
                    &app,
                    TwitchAuthRequiredReason::MissingRequiredScope,
                    message.clone(),
                );
                emit_app_log(&app, AppLogLevel::Warning, message.clone());
                return Err(message);
            }

            {
                let mut auth = state
                    .twitch_auth
                    .lock()
                    .map_err(|error| error.to_string())?;
                if auth.generation != pending.generation {
                    return Err(
                        "新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。"
                            .to_string(),
                    );
                }
                auth.pending = None;
                auth.profile = Some(profile.clone());
                auth.token = Some(TwitchToken {
                    access_token: token.access_token,
                    refresh_token: token.refresh_token,
                    scopes: token_scopes(token.scope, &profile),
                    expires_in: token.expires_in,
                });
                let storage_warning = save_or_storage_warning(&auth);
                emit_twitch_status(
                    &app,
                    TwitchStatusDomain::Auth,
                    TwitchStatus::Connected,
                    Some(format!(
                        "Twitch に {} としてログインしました。",
                        profile.login
                    )),
                );
                emit_app_log(
                    &app,
                    AppLogLevel::Info,
                    format!("Twitch に {} としてログインしました。", profile.login),
                );
                Ok(TwitchAuthPollResult::Authorized {
                    profile,
                    storage_warning,
                })
            }
        }
        Err(PollAuthError::Pending) => Ok(TwitchAuthPollResult::Pending {
            message: {
                ensure_pending_auth_is_current(&state, pending.generation)?;
                clear_poll_in_flight_if_current(&state, pending.generation)?;
                emit_twitch_status(
                    &app,
                    TwitchStatusDomain::Auth,
                    TwitchStatus::Connecting,
                    Some("Twitch の認可完了を待っています。".to_string()),
                );
                "Twitch の認可完了を待っています。ブラウザでコードを入力してください。".to_string()
            },
            interval: pending.interval,
        }),
        Err(PollAuthError::SlowDown) => {
            let interval = pending.interval + 5;
            let mut auth = state
                .twitch_auth
                .lock()
                .map_err(|error| error.to_string())?;
            if auth.generation != pending.generation {
                return Err(
                    "新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。"
                        .to_string(),
                );
            }
            if let Some(stored) = &mut auth.pending {
                stored.interval = interval;
                stored.poll_in_flight = false;
            }
            emit_twitch_status(
                &app,
                TwitchStatusDomain::Auth,
                TwitchStatus::Connecting,
                Some("Twitch 認証の確認間隔を延長しました。".to_string()),
            );
            Ok(TwitchAuthPollResult::SlowDown {
                message: "確認間隔が短すぎます。少し待ってから再確認してください。".to_string(),
                interval,
            })
        }
        Err(PollAuthError::Denied) => {
            ensure_pending_auth_is_current(&state, pending.generation)?;
            clear_pending_if_current(&state, pending.generation)?;
            Ok(TwitchAuthPollResult::Denied {
                message: {
                    emit_twitch_status(
                        &app,
                        TwitchStatusDomain::Auth,
                        TwitchStatus::AuthRequired,
                        Some("Twitch 認証がキャンセルされました。".to_string()),
                    );
                    emit_app_log(
                        &app,
                        AppLogLevel::Warning,
                        "Twitch 認証がキャンセルされました。必要なら再度開始してください。",
                    );
                    "Twitch 認証がキャンセルされました。必要なら再度開始してください。".to_string()
                },
            })
        }
        Err(PollAuthError::Expired) => {
            ensure_pending_auth_is_current(&state, pending.generation)?;
            clear_pending_if_current(&state, pending.generation)?;
            Ok(TwitchAuthPollResult::Expired {
                message: {
                    emit_twitch_status(
                        &app,
                        TwitchStatusDomain::Auth,
                        TwitchStatus::AuthRequired,
                        Some("Twitch 認証コードの期限が切れました。".to_string()),
                    );
                    emit_app_log(
                        &app,
                        AppLogLevel::Warning,
                        "Twitch 認証コードの期限が切れました。再度開始してください。",
                    );
                    "Twitch 認証コードの期限が切れました。再度開始してください。".to_string()
                },
            })
        }
        Err(PollAuthError::Other(error)) => {
            ensure_pending_auth_is_current(&state, pending.generation)?;
            clear_poll_in_flight_if_current(&state, pending.generation)?;
            let message = to_twitch_user_message(error);
            emit_twitch_status(
                &app,
                TwitchStatusDomain::Auth,
                TwitchStatus::Error,
                Some(message.clone()),
            );
            emit_app_log(&app, AppLogLevel::Error, message.clone());
            Err(message)
        }
    }
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn twitch_validate_auth(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<TwitchAuthValidationResult, String> {
    let (generation, access_token, refresh_token, client_id) = {
        let auth = state
            .twitch_auth
            .lock()
            .map_err(|error| error.to_string())?;
        let token = auth
            .token
            .as_ref()
            .ok_or_else(|| "Twitch にログインしていません。".to_string())?;
        let client_id = auth
            .profile
            .as_ref()
            .map(|profile| profile.client_id.clone())
            .filter(|client_id| !client_id.trim().is_empty())
            .or_else(|| Some(default_twitch_client_id()))
            .unwrap_or_default();
        (
            auth.generation,
            token.access_token.clone(),
            token.refresh_token.clone(),
            client_id,
        )
    };

    let profile = match validate_access_token(&access_token).await {
        Ok(validate) => {
            let profile = TwitchUserProfile::from(validate);
            if let Err(error) = ensure_required_twitch_scopes(&profile.scopes) {
                let message = error.to_string();
                ensure_auth_generation_is_current(&state, generation)?;
                clear_missing_scope_twitch_auth(&state, &app, &message)?;
                return Err(message);
            }
            profile
        }
        Err(validate_error) => {
            let token = match refresh_access_token(&client_id, &refresh_token).await {
                Ok(token) => token,
                Err(refresh_error) => {
                    let message = to_twitch_user_message(anyhow::anyhow!(
                        "{validate_error}; {refresh_error}"
                    ));
                    ensure_auth_generation_is_current(&state, generation)?;
                    clear_invalid_twitch_auth(&state, &app, &message)?;
                    return Err(message);
                }
            };
            let profile = match validate_access_token(&token.access_token).await {
                Ok(validate) => {
                    let profile = TwitchUserProfile::from(validate);
                    if let Err(error) = ensure_required_twitch_scopes(&profile.scopes) {
                        let message = error.to_string();
                        ensure_auth_generation_is_current(&state, generation)?;
                        clear_missing_scope_twitch_auth(&state, &app, &message)?;
                        return Err(message);
                    }
                    profile
                }
                Err(error) => {
                    let message = to_twitch_user_message(error);
                    ensure_auth_generation_is_current(&state, generation)?;
                    clear_invalid_twitch_auth(&state, &app, &message)?;
                    return Err(message);
                }
            };
            let mut auth = state
                .twitch_auth
                .lock()
                .map_err(|error| error.to_string())?;
            if auth.generation != generation {
                return Err(
                    "新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。"
                        .to_string(),
                );
            }
            auth.profile = Some(profile.clone());
            auth.token = Some(TwitchToken {
                access_token: token.access_token,
                refresh_token: token.refresh_token,
                scopes: token_scopes(token.scope, &profile),
                expires_in: token.expires_in,
            });
            let storage_warning = save_or_storage_warning(&auth);
            emit_twitch_status(
                &app,
                TwitchStatusDomain::Auth,
                TwitchStatus::Connected,
                Some("Twitch 認証を更新しました。".to_string()),
            );
            emit_app_log(&app, AppLogLevel::Info, "Twitch 認証を更新しました。");
            return Ok(TwitchAuthValidationResult {
                profile,
                storage_warning,
            });
        }
    };

    let mut auth = state
        .twitch_auth
        .lock()
        .map_err(|error| error.to_string())?;
    if auth.generation != generation {
        return Err(
            "新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。".to_string(),
        );
    }
    auth.profile = Some(profile.clone());
    let storage_warning = save_or_storage_warning(&auth);
    emit_twitch_status(
        &app,
        TwitchStatusDomain::Auth,
        TwitchStatus::Connected,
        Some("Twitch 認証は有効です。".to_string()),
    );
    emit_app_log(&app, AppLogLevel::Info, "Twitch 認証は有効です。");
    Ok(TwitchAuthValidationResult {
        profile,
        storage_warning,
    })
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn twitch_get_stored_auth(
    state: tauri::State<'_, AppState>,
) -> Result<Option<TwitchUserProfile>, String> {
    Ok(state
        .twitch_auth
        .lock()
        .map_err(|error| error.to_string())?
        .profile())
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn twitch_connect(
    channel_login: Option<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    let configured_channel = {
        let settings = state.settings.lock().map_err(|error| error.to_string())?;
        settings.twitch.channel_login.trim().to_string()
    };
    let channel_login = channel_login
        .unwrap_or(configured_channel)
        .trim()
        .trim_start_matches('@')
        .to_ascii_lowercase();

    let (access_token, client_id, user_id, own_login, scopes) = {
        let auth = state
            .twitch_auth
            .lock()
            .map_err(|error| error.to_string())?;
        let token = auth
            .token
            .as_ref()
            .ok_or_else(|| "Twitch にログインしてから接続してください。".to_string())?;
        let profile = auth.profile.as_ref().ok_or_else(|| {
            "Twitch のユーザー情報がありません。認証を確認してください。".to_string()
        })?;
        (
            token.access_token.clone(),
            profile.client_id.clone(),
            profile.user_id.clone(),
            profile.login.clone(),
            profile.scopes.clone(),
        )
    };

    if let Err(error) = ensure_required_twitch_scopes(&scopes) {
        let message = error.to_string();
        clear_missing_scope_twitch_auth(&state, &app, &message)?;
        return Err(message);
    }

    let channel_login = if channel_login.is_empty() {
        own_login
    } else {
        channel_login
    };
    let broadcaster = fetch_twitch_user(&client_id, &access_token, &channel_login)
        .await
        .map_err(to_twitch_user_message)?;
    let params = EventSubConnectionParams {
        broadcaster_user_id: broadcaster.id.clone(),
        broadcaster_login: broadcaster.login.clone(),
        user_id,
    };

    {
        let mut connection = state
            .twitch_connection
            .lock()
            .map_err(|error| error.to_string())?;
        if let Some(handle) = connection.take() {
            handle.abort();
        }

        let app_for_task = app.clone();
        let task = tokio::spawn(async move {
            run_eventsub_connection(app_for_task, params).await;
        });
        *connection = Some(TwitchConnectionHandle::new(task));
    }

    emit_twitch_status(
        &app,
        TwitchStatusDomain::Chat,
        TwitchStatus::Connecting,
        Some(format!(
            "Twitch チャンネル {} に接続しています。",
            broadcaster.display_name
        )),
    );
    emit_app_log(
        &app,
        AppLogLevel::Info,
        format!(
            "Twitch チャンネル {} への EventSub 接続を開始しました。",
            broadcaster.login
        ),
    );
    Ok(())
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn twitch_disconnect(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    clear_twitch_auth_state(&state)?;
    emit_twitch_status(
        &app,
        TwitchStatusDomain::Auth,
        TwitchStatus::Disconnected,
        Some("Twitch 連携を解除しました。".to_string()),
    );
    emit_app_log(&app, AppLogLevel::Info, "Twitch 連携を解除しました。");
    Ok(())
}

#[cfg(feature = "app")]
fn clear_twitch_auth_state(state: &tauri::State<'_, AppState>) -> Result<(), String> {
    if let Some(handle) = state
        .twitch_connection
        .lock()
        .map_err(|error| error.to_string())?
        .take()
    {
        handle.abort();
    }

    let mut auth = state
        .twitch_auth
        .lock()
        .map_err(|error| error.to_string())?;
    auth.generation = auth.generation.wrapping_add(1);
    auth.pending = None;
    auth.token = None;
    auth.profile = None;
    TwitchAuthStore::clear().map_err(to_secure_store_user_message)
}

#[cfg(feature = "app")]
fn ensure_pending_auth_is_current(
    state: &tauri::State<'_, AppState>,
    generation: u64,
) -> Result<(), String> {
    let auth = state
        .twitch_auth
        .lock()
        .map_err(|error| error.to_string())?;
    if auth.pending_is_current(generation) {
        Ok(())
    } else {
        Err("新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。".to_string())
    }
}

#[cfg(feature = "app")]
fn ensure_auth_generation_is_current(
    state: &tauri::State<'_, AppState>,
    generation: u64,
) -> Result<(), String> {
    let auth = state
        .twitch_auth
        .lock()
        .map_err(|error| error.to_string())?;
    if auth.generation == generation {
        Ok(())
    } else {
        Err("新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。".to_string())
    }
}

#[cfg(feature = "app")]
fn clear_poll_in_flight_if_current(
    state: &tauri::State<'_, AppState>,
    generation: u64,
) -> Result<(), String> {
    let mut auth = state
        .twitch_auth
        .lock()
        .map_err(|error| error.to_string())?;
    if auth.generation == generation {
        if let Some(pending) = &mut auth.pending {
            pending.poll_in_flight = false;
        }
    }
    Ok(())
}

#[cfg(feature = "app")]
fn clear_pending_if_current(
    state: &tauri::State<'_, AppState>,
    generation: u64,
) -> Result<(), String> {
    let mut auth = state
        .twitch_auth
        .lock()
        .map_err(|error| error.to_string())?;
    if auth.generation == generation {
        auth.pending = None;
    }
    Ok(())
}

#[cfg(feature = "app")]
fn clear_invalid_twitch_auth(
    state: &tauri::State<'_, AppState>,
    app: &tauri::AppHandle<tauri::Wry>,
    error_message: &str,
) -> Result<(), String> {
    clear_twitch_auth_state(state)?;
    let message = format!("Twitch 認証が無効なため、認証状態を解除しました: {error_message}");
    emit_twitch_status(
        app,
        TwitchStatusDomain::Auth,
        TwitchStatus::AuthRequired,
        Some(message.clone()),
    );
    emit_app_log(app, AppLogLevel::Warning, message);
    Ok(())
}

#[cfg(feature = "app")]
fn clear_missing_scope_twitch_auth(
    state: &tauri::State<'_, AppState>,
    app: &tauri::AppHandle<tauri::Wry>,
    error_message: &str,
) -> Result<(), String> {
    clear_twitch_auth_state(state)?;
    emit_twitch_auth_required(
        app,
        TwitchAuthRequiredReason::MissingRequiredScope,
        error_message,
    );
    emit_app_log(app, AppLogLevel::Warning, error_message);
    Ok(())
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn twitch_stop_chat(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    let stopped = state
        .twitch_connection
        .lock()
        .map_err(|error| error.to_string())?
        .take()
        .map(|handle| {
            handle.abort();
        })
        .is_some();

    emit_twitch_status(
        &app,
        TwitchStatusDomain::Chat,
        TwitchStatus::Disconnected,
        Some(if stopped {
            "Twitch チャット受信を停止しました。".to_string()
        } else {
            "Twitch チャット受信は開始されていません。".to_string()
        }),
    );
    emit_app_log(
        &app,
        AppLogLevel::Info,
        if stopped {
            "Twitch チャット受信を停止しました。"
        } else {
            "Twitch チャット受信は開始されていません。"
        },
    );
    Ok(())
}

async fn request_device_code(client_id: &str) -> anyhow::Result<DeviceCodeResponse> {
    let response = reqwest::Client::new()
        .post(TWITCH_DEVICE_URL)
        .form(&[("client_id", client_id), ("scopes", CHAT_READ_SCOPE)])
        .send()
        .await?;

    parse_json_response(response).await
}

async fn poll_device_token(pending: &PendingDeviceAuth) -> Result<TokenResponse, PollAuthError> {
    let response = reqwest::Client::new()
        .post(TWITCH_TOKEN_URL)
        .form(&[
            ("client_id", pending.client_id.as_str()),
            ("scope", CHAT_READ_SCOPE),
            ("device_code", pending.device_code.as_str()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|error| PollAuthError::Other(error.into()))?;

    if response.status().is_success() {
        return response
            .json::<TokenResponse>()
            .await
            .map_err(|error| PollAuthError::Other(error.into()));
    }

    let error = response
        .json::<OAuthErrorResponse>()
        .await
        .map_err(|error| PollAuthError::Other(error.into()))?;
    match oauth_error_code(&error) {
        Some("authorization_pending") => Err(PollAuthError::Pending),
        Some("slow_down") => Err(PollAuthError::SlowDown),
        Some("access_denied") => Err(PollAuthError::Denied),
        Some("expired_token") => Err(PollAuthError::Expired),
        _ => Err(PollAuthError::Other(anyhow::anyhow!(
            "{}",
            error
                .message
                .unwrap_or_else(|| "Twitch 認証に失敗しました。".to_string())
        ))),
    }
}

fn oauth_error_code(error: &OAuthErrorResponse) -> Option<&str> {
    error.error.as_deref().or(error.message.as_deref())
}

async fn refresh_access_token(
    client_id: &str,
    refresh_token: &str,
) -> anyhow::Result<TokenResponse> {
    if client_id.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "Twitch Client ID が見つかりません。再ログインしてください。"
        ));
    }

    let response = reqwest::Client::new()
        .post(TWITCH_TOKEN_URL)
        .form(&[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await?;

    parse_json_response(response).await
}

async fn validate_access_token(access_token: &str) -> anyhow::Result<ValidateResponse> {
    let response = reqwest::Client::new()
        .get(TWITCH_VALIDATE_URL)
        .bearer_auth(access_token)
        .send()
        .await?;

    parse_json_response(response).await
}

async fn fetch_twitch_user(
    client_id: &str,
    access_token: &str,
    login: &str,
) -> anyhow::Result<HelixUser> {
    let response = reqwest::Client::new()
        .get(TWITCH_USERS_URL)
        .query(&[("login", login)])
        .header("Client-Id", client_id)
        .bearer_auth(access_token)
        .send()
        .await?;

    let users = parse_json_response::<HelixUsersResponse>(response).await?;
    users
        .data
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Twitch チャンネル {login} が見つかりません。"))
}

#[cfg(feature = "app")]
async fn run_eventsub_connection(
    app: tauri::AppHandle<tauri::Wry>,
    params: EventSubConnectionParams,
) {
    let mut url = TWITCH_EVENTSUB_WS_URL.to_string();
    let mut subscribe_on_welcome = true;
    let mut reconnect_backoff = EventSubReconnectBackoff::default();
    let mut seen_message_ids = MessageDedupe::new(DEDUPE_CACHE_LIMIT, DEDUPE_CACHE_TTL);

    loop {
        match run_eventsub_session(
            &app,
            &params,
            &url,
            subscribe_on_welcome,
            &mut reconnect_backoff,
            &mut seen_message_ids,
        )
        .await
        {
            Ok(EventSubSessionExit::Reconnect(reconnect_url)) => {
                reconnect_backoff.record_handover_started();
                url = reconnect_url;
                subscribe_on_welcome = false;
                emit_twitch_status(
                    &app,
                    TwitchStatusDomain::Chat,
                    TwitchStatus::Reconnecting,
                    Some("Twitch から再接続要求を受け取りました。".to_string()),
                );
                emit_app_log(
                    &app,
                    AppLogLevel::Warning,
                    "Twitch EventSub の再接続要求を受け取りました。",
                );
            }
            Err(error) => {
                let error_message = error.to_string();
                if error_message.contains("購読が取り消されました") {
                    // The accompanying authRequired status carries the single user-facing
                    // recovery notification. Keep the terminal detail in Logs without
                    // producing a second warning in the renderer.
                    emit_app_log(&app, AppLogLevel::Info, error_message);
                    break;
                }
                let wait_seconds = reconnect_backoff.next_delay_after_failure();
                let message = format!(
                    "Twitch EventSub が切断されました。{} 秒後に再接続します: {error}",
                    wait_seconds
                );
                emit_twitch_status(
                    &app,
                    TwitchStatusDomain::Chat,
                    TwitchStatus::Reconnecting,
                    Some(message.clone()),
                );
                emit_app_log(&app, AppLogLevel::Warning, message);
                tokio::time::sleep(Duration::from_secs(wait_seconds)).await;
                url = TWITCH_EVENTSUB_WS_URL.to_string();
                subscribe_on_welcome = true;
            }
        }
    }
}

#[cfg(feature = "app")]
async fn run_eventsub_session(
    app: &tauri::AppHandle<tauri::Wry>,
    params: &EventSubConnectionParams,
    url: &str,
    subscribe_on_welcome: bool,
    reconnect_backoff: &mut EventSubReconnectBackoff,
    seen_message_ids: &mut MessageDedupe,
) -> anyhow::Result<EventSubSessionExit> {
    emit_twitch_status(
        app,
        TwitchStatusDomain::Chat,
        TwitchStatus::Connecting,
        Some(format!(
            "Twitch チャンネル {} に接続しています。",
            params.broadcaster_login
        )),
    );

    let (mut socket, _) = connect_async(url).await?;
    let mut keepalive_timeout = Duration::from_secs(40);

    loop {
        let next_message =
            tokio::time::timeout(keepalive_timeout + Duration::from_secs(5), socket.next())
                .await
                .map_err(|_| {
                    anyhow::anyhow!("Twitch から keepalive または通知が届きませんでした。")
                })?
                .ok_or_else(|| anyhow::anyhow!("Twitch EventSub WebSocket が閉じられました。"))??;

        match next_message {
            Message::Text(text) => {
                let envelope = serde_json::from_str::<EventSubEnvelope>(&text)?;
                match envelope.metadata.message_type.as_str() {
                    "session_welcome" => {
                        let session = envelope.payload.session.ok_or_else(|| {
                            anyhow::anyhow!("Twitch の welcome に session がありません。")
                        })?;
                        if let Some(seconds) = session.keepalive_timeout_seconds {
                            keepalive_timeout = Duration::from_secs(seconds);
                        }
                        if subscribe_on_welcome {
                            create_chat_message_subscription(app, params, &session.id).await?;
                        }
                        reconnect_backoff.record_session_established();
                        emit_twitch_status(
                            app,
                            TwitchStatusDomain::Chat,
                            TwitchStatus::Connected,
                            Some(format!(
                                "Twitch チャンネル {} に接続しました。",
                                params.broadcaster_login
                            )),
                        );
                        emit_app_log(
                            app,
                            AppLogLevel::Info,
                            format!("Twitch EventSub session {} を開始しました。", session.id),
                        );
                    }
                    "session_keepalive" => {}
                    "session_reconnect" => {
                        let reconnect_url = envelope
                            .payload
                            .session
                            .and_then(|session| session.reconnect_url)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Twitch の reconnect に reconnect_url がありません。"
                                )
                            })?;
                        return Ok(EventSubSessionExit::Reconnect(reconnect_url));
                    }
                    "notification" => {
                        if let Some(normalized) = normalize_chat_message(envelope, Utc::now())? {
                            if let Some(warning) = normalized.timestamp_warning {
                                emit_app_log(app, AppLogLevel::Warning, warning);
                            }
                            let message = normalized.message;
                            let dedupe_id = message.id.clone();
                            if seen_message_ids.insert(dedupe_id) {
                                emit_twitch_chat_message(app, message.clone());
                                if let Err(error) =
                                    enqueue_chat_message_for_speech(app.clone(), message)
                                {
                                    emit_app_log(app, AppLogLevel::Error, error);
                                }
                            }
                        }
                    }
                    "revocation" => {
                        let reason = envelope
                            .payload
                            .subscription
                            .map(|subscription| {
                                format!("{} ({})", subscription.status, subscription.kind)
                            })
                            .unwrap_or_else(|| "理由不明".to_string());
                        emit_twitch_status(
                            app,
                            TwitchStatusDomain::Chat,
                            TwitchStatus::AuthRequired,
                            None,
                        );
                        emit_twitch_status(
                            app,
                            TwitchStatusDomain::Auth,
                            TwitchStatus::AuthRequired,
                            Some(format!(
                                "Twitch EventSub 購読が取り消されたため、再ログインしてください: {reason}"
                            )),
                        );
                        return Err(anyhow::anyhow!(
                            "Twitch EventSub 購読が取り消されました: {reason}"
                        ));
                    }
                    _ => {}
                }
            }
            Message::Ping(payload) => {
                socket.send(Message::Pong(payload)).await?;
            }
            Message::Close(frame) => {
                return Err(anyhow::anyhow!(
                    "Twitch EventSub WebSocket が閉じられました: {:?}",
                    frame
                ));
            }
            _ => {}
        }
    }
}

#[cfg(feature = "app")]
async fn create_chat_message_subscription(
    app: &tauri::AppHandle<tauri::Wry>,
    params: &EventSubConnectionParams,
    session_id: &str,
) -> anyhow::Result<()> {
    let credentials = app
        .state::<AppState>()
        .twitch_auth
        .lock()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .eventsub_credentials()?;
    let subscription_client_id = credentials.client_id.clone();
    let refresh_app = app.clone();
    let refresh_credentials = credentials.clone();

    match retry_eventsub_subscription(
        credentials.access_token,
        |access_token| {
            let client_id = subscription_client_id.clone();
            async move {
                send_chat_message_subscription(params, session_id, &client_id, &access_token).await
            }
        },
        move || {
            let app = refresh_app.clone();
            let credentials = refresh_credentials.clone();
            async move { refresh_eventsub_access_token(&app, &credentials).await }
        },
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(SubscriptionRequestError::Unauthorized) => {
            let message =
                "Twitch 認証を更新しても EventSub 購読が拒否されました。再ログインしてください。";
            clear_eventsub_auth(app, message)?;
            Err(anyhow::anyhow!(message))
        }
        Err(SubscriptionRequestError::Other(error)) => Err(error),
    }
}

#[cfg(feature = "app")]
async fn retry_eventsub_subscription<Subscribe, SubscribeFuture, Refresh, RefreshFuture>(
    access_token: String,
    mut subscribe: Subscribe,
    mut refresh: Refresh,
) -> Result<(), SubscriptionRequestError>
where
    Subscribe: FnMut(String) -> SubscribeFuture,
    SubscribeFuture: std::future::Future<Output = Result<(), SubscriptionRequestError>>,
    Refresh: FnMut() -> RefreshFuture,
    RefreshFuture: std::future::Future<Output = anyhow::Result<String>>,
{
    match subscribe(access_token).await {
        Ok(()) => Ok(()),
        Err(SubscriptionRequestError::Unauthorized) => {
            let refreshed_access_token = refresh().await?;
            subscribe(refreshed_access_token).await
        }
        Err(error) => Err(error),
    }
}

#[cfg(feature = "app")]
async fn refresh_eventsub_access_token(
    app: &tauri::AppHandle<tauri::Wry>,
    credentials: &EventSubAuthCredentials,
) -> anyhow::Result<String> {
    let latest_credentials = app
        .state::<AppState>()
        .twitch_auth
        .lock()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .eventsub_credentials()?;
    if latest_credentials.refresh_token != credentials.refresh_token {
        return Ok(latest_credentials.access_token);
    }

    let refreshed =
        match refresh_access_token(&credentials.client_id, &credentials.refresh_token).await {
            Ok(token) => token,
            Err(error) => {
                let message = to_twitch_user_message(error);
                clear_eventsub_auth(app, &message)?;
                return Err(anyhow::anyhow!(message));
            }
        };

    let refreshed_profile = match validate_access_token(&refreshed.access_token).await {
        Ok(validate) => TwitchUserProfile::from(validate),
        Err(error) => {
            let message = to_twitch_user_message(error);
            clear_eventsub_auth(app, &message)?;
            return Err(anyhow::anyhow!(message));
        }
    };
    if let Err(error) = ensure_required_twitch_scopes(&refreshed_profile.scopes) {
        let message = error.to_string();
        if let Some(access_token) = clear_eventsub_auth_for_missing_scope_if_current(
            app,
            &credentials.refresh_token,
            &message,
        )? {
            // Another EventSub re-subscription refreshed and rotated the credentials
            // while this request was validating its now-stale refresh result.  Its
            // access token is authoritative, so leave that newer authentication in
            // place and retry the subscription with it.
            return Ok(access_token);
        }
        return Err(anyhow::anyhow!(message));
    }

    let (access_token, storage_warning, did_refresh) = {
        let state = app.state::<AppState>();
        let mut auth = state
            .twitch_auth
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let current_credentials = auth.eventsub_credentials()?;
        if current_credentials.refresh_token != credentials.refresh_token {
            (current_credentials.access_token, None, false)
        } else {
            let access_token = auth.replace_token(refreshed, refreshed_profile)?;
            let storage_warning = save_or_storage_warning(&auth);
            (access_token, storage_warning, true)
        }
    };

    if did_refresh {
        emit_app_log(
            app,
            AppLogLevel::Info,
            "Twitch EventSub の再購読前に認証を更新しました。",
        );
    }
    if let Some(warning) = storage_warning {
        emit_app_log(app, AppLogLevel::Warning, warning);
    }
    Ok(access_token)
}

#[cfg(feature = "app")]
fn clear_eventsub_auth(
    app: &tauri::AppHandle<tauri::Wry>,
    error_message: &str,
) -> anyhow::Result<()> {
    let state = app.state::<AppState>();
    clear_invalid_twitch_auth(&state, app, error_message).map_err(|error| anyhow::anyhow!(error))
}

/// Clears an EventSub authentication only when it still belongs to the refresh
/// request that found a missing required scope.  Concurrent EventSub retries can
/// rotate a refresh token while an older request is awaiting `/validate`; clearing
/// unconditionally would discard the newer, valid authentication.
///
/// Returns the newer access token when the credentials have already rotated.
#[cfg(feature = "app")]
fn clear_eventsub_auth_for_missing_scope_if_current(
    app: &tauri::AppHandle<tauri::Wry>,
    expected_refresh_token: &str,
    error_message: &str,
) -> anyhow::Result<Option<String>> {
    let state = app.state::<AppState>();
    let connection_handle = {
        // Keep the same lock ordering as `clear_twitch_auth_state`: connection,
        // then authentication.  The comparison and clearing are one critical
        // section so a rotating refresh cannot be cleared after the comparison.
        let mut connection = state
            .twitch_connection
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let mut auth = state
            .twitch_auth
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        if let Some(access_token) =
            clear_auth_for_eventsub_missing_scope_if_current(&mut auth, expected_refresh_token)?
        {
            return Ok(Some(access_token));
        }

        // Persist the clear while the token lock is held.  Otherwise a concurrent
        // refresh could save a new token and this stale request could erase it.
        TwitchAuthStore::clear()
            .map_err(to_secure_store_user_message)
            .map_err(|error| anyhow::anyhow!(error))?;
        connection.take()
    };

    if let Some(handle) = connection_handle {
        handle.abort();
    }
    emit_twitch_auth_required(
        app,
        TwitchAuthRequiredReason::MissingRequiredScope,
        error_message,
    );
    emit_app_log(app, AppLogLevel::Warning, error_message);
    Ok(None)
}

#[cfg(feature = "app")]
fn clear_auth_for_eventsub_missing_scope_if_current(
    auth: &mut TwitchAuthState,
    expected_refresh_token: &str,
) -> anyhow::Result<Option<String>> {
    let current_credentials = auth.eventsub_credentials()?;
    if current_credentials.refresh_token != expected_refresh_token {
        return Ok(Some(current_credentials.access_token));
    }

    auth.pending = None;
    auth.token = None;
    auth.profile = None;
    Ok(None)
}

#[cfg(feature = "app")]
async fn send_chat_message_subscription(
    params: &EventSubConnectionParams,
    session_id: &str,
    client_id: &str,
    access_token: &str,
) -> Result<(), SubscriptionRequestError> {
    let body = serde_json::json!({
        "type": CHANNEL_CHAT_MESSAGE_TYPE,
        "version": CHANNEL_CHAT_MESSAGE_VERSION,
        "condition": {
            "broadcaster_user_id": params.broadcaster_user_id,
            "user_id": params.user_id,
        },
        "transport": {
            "method": "websocket",
            "session_id": session_id,
        },
    });
    let response = reqwest::Client::new()
        .post(TWITCH_EVENTSUB_SUBSCRIPTIONS_URL)
        .header("Client-Id", client_id)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await
        .map_err(anyhow::Error::from)?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SubscriptionRequestError::Unauthorized);
    }

    parse_json_response::<serde_json::Value>(response).await?;
    Ok(())
}

struct NormalizedChatMessage {
    message: ChatMessage,
    timestamp_warning: Option<String>,
}

fn normalize_chat_message(
    envelope: EventSubEnvelope,
    fallback_received_at: DateTime<Utc>,
) -> anyhow::Result<Option<NormalizedChatMessage>> {
    if envelope.metadata.subscription_type.as_deref() != Some(CHANNEL_CHAT_MESSAGE_TYPE) {
        return Ok(None);
    }

    let event = match envelope.payload.event {
        Some(event) => event,
        None => return Ok(None),
    };
    let event = serde_json::from_value::<EventSubChatMessageEvent>(event)?;
    let (received_at, used_timestamp_fallback) = match envelope
        .metadata
        .message_timestamp
        .as_deref()
        .and_then(|timestamp| DateTime::parse_from_rfc3339(timestamp).ok())
    {
        Some(timestamp) => (timestamp.with_timezone(&Utc), false),
        None => (fallback_received_at, true),
    };
    let id = if event.message_id.is_empty() {
        envelope.metadata.message_id
    } else {
        event.message_id
    };

    let timestamp_warning = used_timestamp_fallback.then(|| {
        format!("Twitch チャット {id} の受信時刻が不正なため、WebSocket 受信時刻を使用しました。")
    });

    Ok(Some(NormalizedChatMessage {
        message: ChatMessage {
            id,
            platform: Platform::Twitch,
            channel_id: event.broadcaster_user_id,
            channel_login: event.broadcaster_user_login,
            user_id: event.chatter_user_id,
            user_login: event.chatter_user_login,
            user_display_name: event.chatter_user_name,
            text: event.message.text,
            fragments: event.message.fragments,
            badges: event.badges,
            received_at,
        },
        timestamp_warning,
    }))
}

fn retry_backoff_seconds(attempt: u64) -> u64 {
    match attempt {
        0 | 1 => 2,
        2 => 5,
        3 => 10,
        _ => 30,
    }
}

struct MessageDedupe {
    limit: usize,
    ttl: Duration,
    seen: HashSet<String>,
    order: VecDeque<(String, Instant)>,
}

impl MessageDedupe {
    fn new(limit: usize, ttl: Duration) -> Self {
        Self {
            limit,
            ttl,
            seen: HashSet::new(),
            order: VecDeque::new(),
        }
    }

    fn insert(&mut self, id: String) -> bool {
        self.insert_at(id, Instant::now())
    }

    fn insert_at(&mut self, id: String, now: Instant) -> bool {
        self.remove_expired(now);

        if !self.seen.insert(id.clone()) {
            return false;
        }

        self.order.push_back((id, now));
        while self.order.len() > self.limit {
            if let Some((old_id, _)) = self.order.pop_front() {
                self.seen.remove(&old_id);
            }
        }
        true
    }

    fn remove_expired(&mut self, now: Instant) {
        while self
            .order
            .front()
            .is_some_and(|(_, seen_at)| now.saturating_duration_since(*seen_at) >= self.ttl)
        {
            if let Some((expired_id, _)) = self.order.pop_front() {
                self.seen.remove(&expired_id);
            }
        }
    }
}

async fn parse_json_response<T>(response: reqwest::Response) -> anyhow::Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    if response.status().is_success() {
        return Ok(response.json::<T>().await?);
    }

    let status = response.status();
    let error = response.json::<OAuthErrorResponse>().await.ok();
    let message = error
        .and_then(|error| error.message.or(error.error))
        .unwrap_or_else(|| status.to_string());

    Err(anyhow::anyhow!(message))
}

fn to_twitch_user_message(error: anyhow::Error) -> String {
    let message = error.to_string();
    if message.contains("401") || message.contains("invalid access token") {
        "Twitch の認証が無効です。再ログインしてください。".to_string()
    } else if message.contains("client") || message.contains("Client") {
        format!("Twitch Client ID を確認してください: {message}")
    } else {
        format!("Twitch 連携でエラーが発生しました: {message}")
    }
}

fn to_secure_store_user_message(error: anyhow::Error) -> String {
    format!(
        "Twitch 認証情報を安全に削除できませんでした。OS の資格情報ストアを確認してから再度解除してください: {error}"
    )
}

#[cfg(feature = "app")]
fn to_session_only_user_message(error: anyhow::Error) -> String {
    format!(
        "OS の資格情報ストアに保存できなかったため、今回の Twitch ログインはこの起動中だけ有効です。認証情報ファイルは作成していません。アプリを再起動したら再ログインしてください。OS の資格情報ストアを確認してください: {error}"
    )
}

#[cfg(feature = "app")]
fn to_secure_store_load_user_message(error: anyhow::Error) -> String {
    format!(
        "OS の資格情報ストアから Twitch 認証情報を読み込めませんでした。資格情報ストアがロックまたは一時的に利用できない可能性があります。OS の資格情報ストアを確認してから再起動するか、Login から再ログインしてください: {error}"
    )
}

#[cfg(feature = "app")]
fn to_legacy_auth_user_message(error: anyhow::Error) -> String {
    format!(
        "以前の平文 Twitch 認証情報（Linux: ~/.rice/twitch-auth.json）を OS の資格情報ストアへ移行できませんでした。安全のため読み込まず、再ログインが必要です。ファイルを削除し、Twitch の「設定と接続」からこのアプリのアクセスを取り消してから再ログインしてください: {error}"
    )
}

#[cfg(feature = "app")]
fn to_auth_recovery_failure_user_message(
    secure_load_error: Option<anyhow::Error>,
    legacy_error: anyhow::Error,
) -> String {
    match secure_load_error {
        Some(secure_error) => format!(
            "{} さらに、{}",
            to_secure_store_load_user_message(secure_error),
            to_legacy_auth_user_message(legacy_error)
        ),
        None => to_legacy_auth_user_message(legacy_error),
    }
}

#[cfg(feature = "app")]
fn to_legacy_cleanup_user_message(error: anyhow::Error) -> String {
    format!(
        "以前の平文 Twitch 認証情報（Linux: ~/.rice/twitch-auth.json）が残っています。ファイルを削除し、Twitch の「設定と接続」からこのアプリのアクセスを取り消して再ログインしてください: {error}"
    )
}

#[cfg(feature = "app")]
fn save_or_storage_warning(auth: &TwitchAuthState) -> Option<String> {
    match TwitchAuthStore::save(auth) {
        Ok(warning) => warning,
        Err(error) => Some(to_secure_store_user_message(error)),
    }
}

fn token_scopes(scopes: Vec<String>, profile: &TwitchUserProfile) -> Vec<String> {
    if scopes.is_empty() {
        profile.scopes.clone()
    } else {
        scopes
    }
}

fn ensure_required_twitch_scopes(scopes: &[String]) -> anyhow::Result<()> {
    let missing_scopes = REQUIRED_TWITCH_SCOPES
        .iter()
        .filter(|required_scope| !scopes.iter().any(|scope| scope == **required_scope))
        .copied()
        .collect::<Vec<_>>();

    if missing_scopes.is_empty() {
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "Twitch 認証に必要な権限がありません: {}。Login から再ログインし、{} を許可してください。",
        missing_scopes.join(", "),
        missing_scopes.join(", "),
    ))
}

enum PollAuthError {
    Pending,
    SlowDown,
    Denied,
    Expired,
    Other(anyhow::Error),
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "app")]
    use super::{
        clear_auth_for_eventsub_missing_scope_if_current, restore_stored_auth,
        retry_eventsub_subscription, AuthSecretStore, AuthStorage, EventSubReconnectBackoff,
        StoredTwitchAuth, SubscriptionRequestError, TokenResponse, TwitchAuthState, TwitchToken,
        TwitchUserProfile, EVENTSUB_BACKOFF_RESET_STABLE_DURATION,
    };
    use super::{
        ensure_required_twitch_scopes, normalize_chat_message, oauth_error_code,
        retry_backoff_seconds, EventSubEnvelope, MessageDedupe, OAuthErrorResponse,
    };
    use chrono::{DateTime, Utc};
    use std::{
        cell::RefCell,
        time::{Duration, Instant},
    };

    #[derive(Default)]
    struct FakeAuthSecretStore {
        secret: RefCell<Option<String>>,
        fail_load: bool,
        fail_save: bool,
        fail_clear: bool,
        save_calls: RefCell<usize>,
    }

    impl FakeAuthSecretStore {
        fn with_secret(secret: String) -> Self {
            Self {
                secret: RefCell::new(Some(secret)),
                ..Self::default()
            }
        }
    }

    impl AuthSecretStore for FakeAuthSecretStore {
        fn load_secret(&self) -> anyhow::Result<Option<String>> {
            if self.fail_load {
                return Err(anyhow::anyhow!("fake secure-store read failure"));
            }
            Ok(self.secret.borrow().clone())
        }

        fn save_secret(&self, secret: &str) -> anyhow::Result<()> {
            *self.save_calls.borrow_mut() += 1;
            if self.fail_save {
                return Err(anyhow::anyhow!("fake secure-store write failure"));
            }
            *self.secret.borrow_mut() = Some(secret.to_string());
            Ok(())
        }

        fn clear_secret(&self) -> anyhow::Result<()> {
            if self.fail_clear {
                return Err(anyhow::anyhow!("fake secure-store clear failure"));
            }
            *self.secret.borrow_mut() = None;
            Ok(())
        }
    }

    fn stored_auth_secret() -> String {
        serde_json::to_string(&StoredTwitchAuth {
            client_id: "client-id".to_string(),
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            scopes: vec!["user:read:chat".to_string()],
            expires_in: 3600,
            profile: TwitchUserProfile {
                user_id: "user-id".to_string(),
                login: "viewer".to_string(),
                client_id: "client-id".to_string(),
                scopes: vec!["user:read:chat".to_string()],
                expires_in: 3600,
            },
        })
        .unwrap()
    }

    fn twitch_auth_state() -> TwitchAuthState {
        TwitchAuthState {
            generation: 0,
            pending: None,
            token: Some(TwitchToken {
                access_token: "access-token".to_string(),
                refresh_token: "refresh-token".to_string(),
                scopes: vec!["user:read:chat".to_string()],
                expires_in: 3600,
            }),
            profile: Some(TwitchUserProfile {
                user_id: "user-id".to_string(),
                login: "viewer".to_string(),
                client_id: "client-id".to_string(),
                scopes: vec!["user:read:chat".to_string()],
                expires_in: 3600,
            }),
        }
    }

    #[test]
    fn newer_auth_generation_invalidates_an_in_flight_poll_and_credentials() {
        let mut auth = twitch_auth_state();
        auth.pending = Some(super::PendingDeviceAuth {
            generation: auth.generation,
            poll_in_flight: true,
            client_id: "client-id".to_string(),
            device_code: "device-a".to_string(),
            interval: 5,
        });
        let stale_generation = auth.generation;

        let current_generation = auth.invalidate_operations();
        auth.token = None;
        auth.profile = None;

        assert_ne!(stale_generation, current_generation);
        assert!(!auth.pending_is_current(stale_generation));
        assert!(auth.pending.is_none());
        assert!(auth.token.is_none());
        assert!(auth.profile.is_none());
    }

    fn scopes_without_chat_read() -> Vec<String> {
        vec!["user:read:email".to_string()]
    }

    #[test]
    fn initial_authorization_requires_chat_read_scope() {
        let error = ensure_required_twitch_scopes(&scopes_without_chat_read()).unwrap_err();

        assert!(error.to_string().contains("user:read:chat"));
        assert!(error.to_string().contains("再ログイン"));
    }

    #[cfg(feature = "app")]
    #[test]
    fn stored_auth_without_chat_read_scope_is_not_restored() {
        let secret = serde_json::to_string(&StoredTwitchAuth {
            client_id: "client-id".to_string(),
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            scopes: scopes_without_chat_read(),
            expires_in: 3600,
            profile: TwitchUserProfile {
                user_id: "user-id".to_string(),
                login: "viewer".to_string(),
                client_id: "client-id".to_string(),
                scopes: scopes_without_chat_read(),
                expires_in: 3600,
            },
        })
        .unwrap();

        let error = restore_stored_auth(&secret).unwrap_err();

        assert!(error.to_string().contains("user:read:chat"));
    }

    #[test]
    fn refreshed_authorization_requires_chat_read_scope() {
        let error = ensure_required_twitch_scopes(&scopes_without_chat_read()).unwrap_err();

        assert!(error.to_string().contains("user:read:chat"));
    }

    #[cfg(feature = "app")]
    #[test]
    fn eventsub_credentials_reject_missing_chat_read_scope_before_subscription() {
        let mut auth = twitch_auth_state();
        auth.profile.as_mut().unwrap().scopes = scopes_without_chat_read();

        let error = auth.eventsub_credentials().unwrap_err();

        assert!(error.to_string().contains("user:read:chat"));
    }

    fn process_session(
        dedupe: &mut MessageDedupe,
        message_ids: &[&str],
        received_at: Instant,
    ) -> Vec<bool> {
        message_ids
            .iter()
            .map(|id| dedupe.insert_at((*id).to_string(), received_at))
            .collect()
    }

    #[test]
    fn reads_device_flow_status_from_twitch_message_field() {
        let response = serde_json::from_str::<OAuthErrorResponse>(
            r#"{"status":400,"message":"authorization_pending"}"#,
        )
        .unwrap();

        assert_eq!(oauth_error_code(&response), Some("authorization_pending"));
    }

    #[test]
    fn reads_standard_oauth_error_field_when_present() {
        let response = serde_json::from_str::<OAuthErrorResponse>(
            r#"{"error":"slow_down","message":"wait before polling again"}"#,
        )
        .unwrap();

        assert_eq!(oauth_error_code(&response), Some("slow_down"));
    }

    fn utc_timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn chat_fixture_with_timestamp(timestamp: Option<&str>) -> EventSubEnvelope {
        let mut fixture = serde_json::from_str::<serde_json::Value>(include_str!(
            "fixtures/channel_chat_message.json"
        ))
        .unwrap();
        let metadata = fixture["metadata"].as_object_mut().unwrap();
        match timestamp {
            Some(timestamp) => {
                metadata.insert(
                    "message_timestamp".to_string(),
                    serde_json::Value::String(timestamp.to_string()),
                );
            }
            None => {
                metadata.remove("message_timestamp");
            }
        }
        serde_json::from_value(fixture).unwrap()
    }

    #[test]
    fn parses_channel_chat_message_fixture() {
        let fixture = include_str!("fixtures/channel_chat_message.json");
        let envelope = serde_json::from_str::<EventSubEnvelope>(fixture).unwrap();
        let normalized =
            normalize_chat_message(envelope, utc_timestamp("2026-08-15T12:34:56.789Z"))
                .unwrap()
                .unwrap();
        assert!(normalized.timestamp_warning.is_none());
        let message = normalized.message;

        assert_eq!(message.id, "cc106a89-1814-919d-454c-f4f2f970aae7");
        assert_eq!(message.channel_id, "1971641");
        assert_eq!(message.channel_login, "streamer");
        assert_eq!(message.user_id, "4145994");
        assert_eq!(message.user_login, "viewer32");
        assert_eq!(message.user_display_name, "viewer32");
        assert_eq!(message.text, "Hi chat Kappa");
        assert_eq!(message.fragments.len(), 2);
        assert_eq!(message.fragments[1].kind, "emote");
        assert_eq!(message.fragments[1].emote.as_ref().unwrap().id, "25");
        assert_eq!(message.badges[0].set_id, "broadcaster");
        assert_eq!(
            message.received_at,
            utc_timestamp("2023-11-06T18:11:47.492253549Z")
        );
    }

    #[test]
    fn normalizes_offset_timestamp_to_utc_and_serializes_the_tauri_field_contract() {
        let normalized = normalize_chat_message(
            chat_fixture_with_timestamp(Some("2026-08-15T21:34:56.789123456+09:00")),
            utc_timestamp("2026-08-15T00:00:00Z"),
        )
        .unwrap()
        .unwrap();

        assert!(normalized.timestamp_warning.is_none());
        assert_eq!(
            normalized.message.received_at,
            utc_timestamp("2026-08-15T12:34:56.789123456Z")
        );
        let serialized = serde_json::to_value(&normalized.message).unwrap();
        assert_eq!(
            serialized["receivedAt"],
            serde_json::json!("2026-08-15T12:34:56.789123456Z")
        );
        assert!(serialized.get("received_at").is_none());
    }

    #[test]
    fn falls_back_to_websocket_receive_time_for_missing_empty_naive_and_invalid_timestamps() {
        let fallback = utc_timestamp("2026-08-15T12:34:56.789Z");
        let cases = [
            ("missing", None),
            ("empty", Some("")),
            ("naive", Some("2026-08-15T12:34:56")),
            ("invalid", Some("invalid-timestamp")),
        ];

        for (case_name, timestamp) in cases {
            let normalized =
                normalize_chat_message(chat_fixture_with_timestamp(timestamp), fallback)
                    .unwrap()
                    .unwrap();

            assert_eq!(normalized.message.received_at, fallback, "{case_name}");
            assert!(
                normalized
                    .timestamp_warning
                    .as_deref()
                    .is_some_and(|warning| warning.contains("WebSocket 受信時刻")),
                "{case_name}"
            );
        }
    }

    #[test]
    fn message_dedupe_rejects_duplicate_ids() {
        let mut dedupe = MessageDedupe::new(2, Duration::from_secs(60));

        assert!(dedupe.insert("a".to_string()));
        assert!(!dedupe.insert("a".to_string()));
        assert!(dedupe.insert("b".to_string()));
        assert!(dedupe.insert("c".to_string()));
        assert!(dedupe.insert("a".to_string()));
    }

    #[test]
    fn message_dedupe_expires_ids_after_ttl() {
        let started_at = Instant::now();
        let mut dedupe = MessageDedupe::new(10, Duration::from_secs(60));

        assert!(dedupe.insert_at("a".to_string(), started_at));
        assert!(!dedupe.insert_at("a".to_string(), started_at + Duration::from_secs(59)));
        assert!(dedupe.insert_at("a".to_string(), started_at + Duration::from_secs(60)));
    }

    #[test]
    fn normal_reconnect_keeps_message_dedupe() {
        let started_at = Instant::now();
        let mut connection_dedupe = MessageDedupe::new(10, Duration::from_secs(60));

        let first_session =
            process_session(&mut connection_dedupe, &["before-reconnect"], started_at);
        let reconnected_session = process_session(
            &mut connection_dedupe,
            &["before-reconnect", "after-reconnect"],
            started_at + Duration::from_secs(2),
        );

        assert_eq!(first_session, vec![true]);
        assert_eq!(reconnected_session, vec![false, true]);
    }

    #[test]
    fn reconnect_handover_keeps_message_dedupe() {
        let started_at = Instant::now();
        let mut connection_dedupe = MessageDedupe::new(10, Duration::from_secs(60));

        let original_session =
            process_session(&mut connection_dedupe, &["before-handover"], started_at);
        let handover_session = process_session(
            &mut connection_dedupe,
            &["before-handover", "after-handover"],
            started_at + Duration::from_millis(100),
        );

        assert_eq!(original_session, vec![true]);
        assert_eq!(handover_session, vec![false, true]);
    }

    #[test]
    fn retry_backoff_caps_after_repeated_reconnects() {
        assert_eq!(retry_backoff_seconds(0), 2);
        assert_eq!(retry_backoff_seconds(1), 2);
        assert_eq!(retry_backoff_seconds(2), 5);
        assert_eq!(retry_backoff_seconds(3), 10);
        assert_eq!(retry_backoff_seconds(4), 30);
        assert_eq!(retry_backoff_seconds(10), 30);
    }

    #[cfg(feature = "app")]
    #[test]
    fn eventsub_backoff_resets_only_after_a_stable_established_session() {
        let started_at = Instant::now();
        let mut backoff = EventSubReconnectBackoff::default();

        assert_eq!(backoff.next_delay_after_failure_at(started_at), 2);
        assert_eq!(
            backoff.next_delay_after_failure_at(started_at + Duration::from_secs(2)),
            5
        );

        // A welcome/subscription that immediately fails must preserve the
        // accumulated delay, so a flaky endpoint cannot cause a retry storm.
        let first_welcome_at = started_at + Duration::from_secs(3);
        backoff.record_session_established_at(first_welcome_at);
        assert_eq!(
            backoff.next_delay_after_failure_at(first_welcome_at + Duration::from_secs(1)),
            10
        );

        // Once a welcome (and, for a normal connection, its subscription) has
        // remained healthy for the stability window, the next fault is a new
        // failure sequence and returns to the shortest retry delay.
        let stable_welcome_at = started_at + Duration::from_secs(10);
        backoff.record_session_established_at(stable_welcome_at);
        assert_eq!(
            backoff.next_delay_after_failure_at(
                stable_welcome_at + EVENTSUB_BACKOFF_RESET_STABLE_DURATION
            ),
            2
        );
    }

    #[cfg(feature = "app")]
    #[test]
    fn eventsub_backoff_keeps_growing_without_an_established_session() {
        let started_at = Instant::now();
        let mut backoff = EventSubReconnectBackoff::default();

        assert_eq!(backoff.next_delay_after_failure_at(started_at), 2);
        assert_eq!(
            backoff.next_delay_after_failure_at(started_at + Duration::from_secs(2)),
            5
        );
        assert_eq!(
            backoff.next_delay_after_failure_at(started_at + Duration::from_secs(7)),
            10
        );
        assert_eq!(
            backoff.next_delay_after_failure_at(started_at + Duration::from_secs(17)),
            30
        );
        assert_eq!(
            backoff.next_delay_after_failure_at(started_at + Duration::from_secs(47)),
            30
        );
    }

    #[cfg(feature = "app")]
    #[test]
    fn eventsub_handover_preserves_backoff_until_the_new_session_is_stable() {
        let started_at = Instant::now();
        let mut backoff = EventSubReconnectBackoff::default();

        assert_eq!(backoff.next_delay_after_failure_at(started_at), 2);
        assert_eq!(
            backoff.next_delay_after_failure_at(started_at + Duration::from_secs(2)),
            5
        );

        // A server-requested handover itself does not reset the retry budget.
        // Its new welcome is the lifecycle point that can establish a session.
        let handover_welcome_at = started_at + Duration::from_secs(7);
        backoff.record_session_established_at(handover_welcome_at);
        assert_eq!(
            backoff.next_delay_after_failure_at(handover_welcome_at + Duration::from_secs(1)),
            10
        );
    }

    #[cfg(feature = "app")]
    #[test]
    fn eventsub_handover_failure_before_new_welcome_ignores_the_old_stable_session() {
        let started_at = Instant::now();
        let mut backoff = EventSubReconnectBackoff::default();

        assert_eq!(backoff.next_delay_after_failure_at(started_at), 2);
        assert_eq!(
            backoff.next_delay_after_failure_at(started_at + Duration::from_secs(2)),
            5
        );

        let old_welcome_at = started_at + Duration::from_secs(3);
        backoff.record_session_established_at(old_welcome_at);
        let handover_started_at = old_welcome_at + EVENTSUB_BACKOFF_RESET_STABLE_DURATION;
        backoff.record_handover_started();

        // A handover connection failure before its welcome must continue the
        // existing failure sequence, even when the old session was stable.
        assert_eq!(
            backoff.next_delay_after_failure_at(handover_started_at + Duration::from_secs(1)),
            10
        );
    }

    #[cfg(feature = "app")]
    #[tokio::test]
    async fn eventsub_reconnect_with_expired_access_token_refreshes_and_retries_once() {
        let attempted_tokens = RefCell::new(Vec::new());
        let refresh_calls = RefCell::new(0);

        let result = retry_eventsub_subscription(
            "expired-access-token".to_string(),
            |access_token| {
                attempted_tokens.borrow_mut().push(access_token.clone());
                async move {
                    if access_token == "expired-access-token" {
                        Err(SubscriptionRequestError::Unauthorized)
                    } else {
                        Ok(())
                    }
                }
            },
            || {
                *refresh_calls.borrow_mut() += 1;
                async { Ok("refreshed-access-token".to_string()) }
            },
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(
            *attempted_tokens.borrow(),
            vec!["expired-access-token", "refreshed-access-token"]
        );
        assert_eq!(*refresh_calls.borrow(), 1);
    }

    #[cfg(feature = "app")]
    #[tokio::test]
    async fn eventsub_refresh_failure_does_not_retry_the_subscription() {
        let attempts = RefCell::new(0);
        let result = retry_eventsub_subscription(
            "expired-access-token".to_string(),
            |_| {
                *attempts.borrow_mut() += 1;
                async { Err(SubscriptionRequestError::Unauthorized) }
            },
            || async { Err(anyhow::anyhow!("refresh token was revoked")) },
        )
        .await;

        assert_eq!(*attempts.borrow(), 1);
        assert!(
            matches!(result, Err(SubscriptionRequestError::Other(error)) if error.to_string().contains("revoked"))
        );
    }

    #[cfg(feature = "app")]
    #[test]
    fn eventsub_credentials_use_the_refreshed_token_and_persist_refresh_rotation() {
        let mut auth = twitch_auth_state();
        assert_eq!(
            auth.eventsub_credentials().unwrap().access_token,
            "access-token"
        );

        auth.replace_token(
            TokenResponse {
                access_token: "refreshed-access-token".to_string(),
                refresh_token: "rotated-refresh-token".to_string(),
                scope: vec!["user:read:chat".to_string()],
                expires_in: 7200,
            },
            TwitchUserProfile {
                user_id: "user-id".to_string(),
                login: "viewer".to_string(),
                client_id: "client-id".to_string(),
                scopes: vec!["user:read:chat".to_string()],
                expires_in: 7200,
            },
        )
        .unwrap();

        let stored = auth.stored_auth().unwrap();
        assert_eq!(
            auth.eventsub_credentials().unwrap().access_token,
            "refreshed-access-token"
        );
        assert_eq!(stored.access_token, "refreshed-access-token");
        assert_eq!(stored.refresh_token, "rotated-refresh-token");
        assert_eq!(stored.expires_in, 7200);

        let secure = FakeAuthSecretStore::default();
        let legacy = FakeAuthSecretStore::default();
        let storage = AuthStorage {
            secure: &secure,
            legacy: &legacy,
        };
        assert_eq!(storage.save(&auth).unwrap(), None);
        let persisted =
            serde_json::from_str::<StoredTwitchAuth>(secure.secret.borrow().as_deref().unwrap())
                .unwrap();
        assert_eq!(persisted.refresh_token, "rotated-refresh-token");
    }

    #[cfg(feature = "app")]
    #[test]
    fn eventsub_refresh_rejects_a_new_token_without_chat_read_scope() {
        let mut auth = twitch_auth_state();
        let error = auth
            .replace_token(
                TokenResponse {
                    access_token: "refreshed-access-token".to_string(),
                    refresh_token: "rotated-refresh-token".to_string(),
                    // Twitch can omit scope from a refresh response, so this must not
                    // fall back to the previously stored profile's scopes.
                    scope: Vec::new(),
                    expires_in: 7200,
                },
                TwitchUserProfile {
                    user_id: "user-id".to_string(),
                    login: "viewer".to_string(),
                    client_id: "client-id".to_string(),
                    scopes: scopes_without_chat_read(),
                    expires_in: 7200,
                },
            )
            .unwrap_err();

        assert!(error.to_string().contains("user:read:chat"));
        assert_eq!(
            auth.eventsub_credentials().unwrap().access_token,
            "access-token"
        );
    }

    #[cfg(feature = "app")]
    #[test]
    fn stale_eventsub_scope_failure_keeps_rotated_authentication() {
        let mut auth = twitch_auth_state();
        let stale_refresh_token = auth.eventsub_credentials().unwrap().refresh_token;

        // Simulate a second EventSub re-subscription completing its refresh while
        // the first one is awaiting validation of a scope-deficient token.
        auth.replace_token(
            TokenResponse {
                access_token: "newer-access-token".to_string(),
                refresh_token: "newer-refresh-token".to_string(),
                scope: vec!["user:read:chat".to_string()],
                expires_in: 7200,
            },
            TwitchUserProfile {
                user_id: "user-id".to_string(),
                login: "viewer".to_string(),
                client_id: "client-id".to_string(),
                scopes: vec!["user:read:chat".to_string()],
                expires_in: 7200,
            },
        )
        .unwrap();

        let access_token =
            clear_auth_for_eventsub_missing_scope_if_current(&mut auth, &stale_refresh_token)
                .unwrap();

        assert_eq!(access_token.as_deref(), Some("newer-access-token"));
        let current = auth.eventsub_credentials().unwrap();
        assert_eq!(current.access_token, "newer-access-token");
        assert_eq!(current.refresh_token, "newer-refresh-token");
    }

    #[test]
    fn saves_auth_to_the_secure_store_when_available() {
        let secure = FakeAuthSecretStore::default();
        let legacy = FakeAuthSecretStore::default();
        let storage = AuthStorage {
            secure: &secure,
            legacy: &legacy,
        };

        assert_eq!(storage.save(&twitch_auth_state()).unwrap(), None);
        assert!(secure.secret.borrow().is_some());
        assert!(legacy.secret.borrow().is_none());
        assert_eq!(*legacy.save_calls.borrow(), 0);
    }

    #[test]
    fn keeps_auth_session_only_when_secure_store_write_fails() {
        let secure = FakeAuthSecretStore {
            fail_save: true,
            ..FakeAuthSecretStore::default()
        };
        let legacy = FakeAuthSecretStore::default();
        let storage = AuthStorage {
            secure: &secure,
            legacy: &legacy,
        };

        let warning = storage.save(&twitch_auth_state()).unwrap().unwrap();

        assert!(warning.contains("この起動中だけ有効"));
        assert!(warning.contains("認証情報ファイルは作成していません"));
        assert!(secure.secret.borrow().is_none());
        assert!(legacy.secret.borrow().is_none());
        assert_eq!(*legacy.save_calls.borrow(), 0);
    }

    #[test]
    fn migrates_existing_legacy_auth_after_secure_store_recovers() {
        let legacy_secret = stored_auth_secret();
        let secure = FakeAuthSecretStore::default();
        let legacy = FakeAuthSecretStore::with_secret(legacy_secret.clone());
        let storage = AuthStorage {
            secure: &secure,
            legacy: &legacy,
        };

        let restored = storage.load();

        assert_eq!(restored.auth.unwrap().profile().unwrap().login, "viewer");
        assert!(restored
            .storage_warning
            .unwrap()
            .contains("移行し、平文ファイルを削除"));
        assert_eq!(
            secure.secret.borrow().as_deref(),
            Some(legacy_secret.as_str())
        );
        assert!(legacy.secret.borrow().is_none());
    }

    #[test]
    fn warns_when_secure_store_cannot_be_read_and_no_legacy_auth_exists() {
        let secure = FakeAuthSecretStore {
            fail_load: true,
            ..FakeAuthSecretStore::default()
        };
        let legacy = FakeAuthSecretStore::default();
        let storage = AuthStorage {
            secure: &secure,
            legacy: &legacy,
        };

        let restored = storage.load();

        assert!(restored.auth.is_none());
        let warning = restored.storage_warning.unwrap();
        assert!(warning.contains("資格情報ストアから Twitch 認証情報を読み込めません"));
        assert!(warning.contains("fake secure-store read failure"));
        assert!(warning.contains("資格情報ストアを確認"));
        assert!(warning.contains("再ログイン"));
    }

    #[test]
    fn leaves_legacy_auth_unread_when_migration_is_rejected() {
        let secure = FakeAuthSecretStore {
            fail_save: true,
            ..FakeAuthSecretStore::default()
        };
        let legacy = FakeAuthSecretStore::with_secret(stored_auth_secret());
        let storage = AuthStorage {
            secure: &secure,
            legacy: &legacy,
        };

        let restored = storage.load();

        assert!(restored.auth.is_none());
        let warning = restored.storage_warning.unwrap();
        assert!(warning.contains("安全のため読み込まず"));
        assert!(warning.contains("ファイルを削除"));
        assert!(warning.contains("アクセスを取り消し"));
        assert!(legacy.secret.borrow().is_some());
    }

    #[test]
    fn logout_clears_secure_and_legacy_auth_state() {
        let secure = FakeAuthSecretStore::with_secret(stored_auth_secret());
        let legacy = FakeAuthSecretStore::with_secret(stored_auth_secret());
        let storage = AuthStorage {
            secure: &secure,
            legacy: &legacy,
        };

        storage.clear().unwrap();

        assert!(secure.secret.borrow().is_none());
        assert!(legacy.secret.borrow().is_none());
    }
}
