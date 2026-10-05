//! Twitch facade: command/event names stay stable while responsibilities move
//! behind private module boundaries. Auth/EventSub service extraction follows.
mod dedupe;
mod error;
mod model;
mod normalization;

#[cfg(any(feature = "app", test))]
use dedupe::MessageDedupe;
use error::*;
pub use model::*;
#[cfg(feature = "app")]
use normalization::EventSubSession;
#[cfg(any(feature = "app", test))]
use normalization::{normalize_chat_message, EventSubEnvelope};

#[cfg(feature = "app")]
use crate::app_events::{
    emit_app_log, emit_twitch_auth_required, emit_twitch_chat_message, emit_twitch_chat_status,
    emit_twitch_status, AppLogLevel, TwitchActiveConnection, TwitchAuthRequiredReason,
    TwitchStatus, TwitchStatusDomain,
};
#[cfg(feature = "app")]
use crate::settings::{default_twitch_client_id, AppState};
#[cfg(feature = "app")]
use crate::speech::enqueue_chat_message_for_speech;
#[cfg(feature = "app")]
use chrono::{DateTime, Utc};
#[cfg(feature = "app")]
use futures_util::{FutureExt, SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
#[cfg(all(test, feature = "app"))]
use std::collections::VecDeque;
#[cfg(feature = "app")]
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
#[cfg(feature = "app")]
use std::time::Instant;
#[cfg(all(feature = "app", target_os = "linux"))]
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
};
#[cfg(feature = "app")]
use tauri::Manager;
#[cfg(feature = "app")]
use tokio::net::TcpStream;
#[cfg(feature = "app")]
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

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
#[cfg(feature = "app")]
const EVENTSUB_RECONNECT_HANDOVER_TIMEOUT: Duration = Duration::from_secs(25);
const CHAT_READ_SCOPE: &str = "user:read:chat";
const REQUIRED_TWITCH_SCOPES: &[&str] = &[CHAT_READ_SCOPE];
const KEYRING_SERVICE: &str = "rice.twitch.oauth";
const KEYRING_ACCOUNT: &str = "default";
const CHANNEL_CHAT_MESSAGE_TYPE: &str = "channel.chat.message";
const CHANNEL_CHAT_MESSAGE_VERSION: &str = "1";
const DEDUPE_CACHE_LIMIT: usize = 5_000;
const DEDUPE_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const TWITCH_HTTP_TIMEOUT: Duration = Duration::from_secs(15);
#[cfg(feature = "app")]
const TWITCH_WS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(feature = "app")]
static NEXT_TWITCH_CONNECTION_GENERATION: AtomicU64 = AtomicU64::new(1);
#[cfg(all(feature = "app", target_os = "linux"))]
const LEGACY_AUTH_DIR: &str = ".rice";
#[cfg(all(feature = "app", target_os = "linux"))]
const LEGACY_AUTH_FILE: &str = "twitch-auth.json";

#[cfg(feature = "app")]
#[derive(Debug)]
pub struct TwitchConnectionHandle {
    generation: u64,
    task: tokio::task::JoinHandle<()>,
}

#[cfg(feature = "app")]
impl TwitchConnectionHandle {
    fn new(generation: u64, task: tokio::task::JoinHandle<()>) -> Self {
        Self { generation, task }
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
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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
        #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
}

#[derive(Debug, Deserialize)]
struct OAuthErrorResponse {
    message: Option<String>,
    error: Option<String>,
}

#[cfg(feature = "app")]
#[derive(Debug, Clone)]
struct EventSubConnectionParams {
    generation: u64,
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
type EventSubSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[cfg(feature = "app")]
enum EventSubFrameAction {
    Continue,
    Activity,
    Welcome(EventSubSession),
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
pub(crate) trait AuthCredentialStore: Send + Sync {
    fn load(&self) -> AuthLoadResult;
    fn save(&self, auth: &TwitchAuthState) -> anyhow::Result<Option<String>>;
    fn clear(&self) -> anyhow::Result<()>;
}

#[cfg(feature = "app")]
#[derive(Clone)]
pub(crate) struct TwitchAuthStore {
    backend: std::sync::Arc<dyn AuthCredentialStore>,
    io_lock: std::sync::Arc<std::sync::Mutex<()>>,
}

#[cfg(feature = "app")]
impl Default for TwitchAuthStore {
    fn default() -> Self {
        Self::with_backend(std::sync::Arc::new(SystemAuthCredentialStore))
    }
}

#[cfg(feature = "app")]
impl TwitchAuthStore {
    pub(crate) fn with_backend(backend: std::sync::Arc<dyn AuthCredentialStore>) -> Self {
        Self {
            backend,
            io_lock: std::sync::Arc::new(std::sync::Mutex::new(())),
        }
    }

    pub(crate) async fn load(&self) -> anyhow::Result<AuthLoadResult> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.load_sync())
            .await
            .map_err(anyhow::Error::from)
    }

    fn load_sync(&self) -> AuthLoadResult {
        let _io_guard = self.io_lock.lock().expect("auth storage mutex poisoned");
        self.backend.load()
    }

    async fn save_if_current(
        &self,
        auth_state: std::sync::Arc<std::sync::Mutex<TwitchAuthState>>,
        generation: u64,
        auth: TwitchAuthState,
    ) -> anyhow::Result<AuthSaveOutcome> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.save_if_current_sync(&auth_state, generation, &auth)
        })
        .await
        .map_err(anyhow::Error::from)?
    }

    fn save_if_current_sync(
        &self,
        auth_state: &std::sync::Mutex<TwitchAuthState>,
        generation: u64,
        auth: &TwitchAuthState,
    ) -> anyhow::Result<AuthSaveOutcome> {
        // Serialize save and clear operations. The generation check is made
        // after acquiring this lock, so a save queued behind logout cannot
        // resurrect credentials that logout has already removed.
        let _io_guard = self
            .io_lock
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let is_current = {
            let current = auth_state
                .lock()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            current.generation == generation
        };
        if !is_current {
            return Ok(AuthSaveOutcome::Stale);
        }
        self.backend.save(auth).map(AuthSaveOutcome::Saved)
    }

    async fn clear_if_current(
        &self,
        auth_state: std::sync::Arc<std::sync::Mutex<TwitchAuthState>>,
        generation: u64,
    ) -> anyhow::Result<AuthClearOutcome> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.clear_if_current_sync(&auth_state, generation))
            .await
            .map_err(anyhow::Error::from)?
    }

    fn clear_if_current_sync(
        &self,
        auth_state: &std::sync::Mutex<TwitchAuthState>,
        generation: u64,
    ) -> anyhow::Result<AuthClearOutcome> {
        let _io_guard = self
            .io_lock
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        if auth_state
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?
            .generation
            != generation
        {
            return Ok(AuthClearOutcome::Stale);
        }
        self.backend.clear()?;
        // The backend call can block after the pre-clear comparison. Keep the
        // I/O lock while checking once more so a newer auth/save waits to write
        // after this old clear, and callers never tear down its connection.
        if auth_state
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?
            .generation
            != generation
        {
            Ok(AuthClearOutcome::StaleAfterClear)
        } else {
            Ok(AuthClearOutcome::Cleared)
        }
    }
}

#[cfg(feature = "app")]
#[derive(Debug, Clone, PartialEq, Eq)]
enum AuthSaveOutcome {
    Saved(Option<String>),
    Stale,
}

#[cfg(feature = "app")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthClearOutcome {
    Cleared,
    Stale,
    StaleAfterClear,
}

#[cfg(feature = "app")]
struct SystemAuthCredentialStore;

#[cfg(feature = "app")]
impl AuthCredentialStore for SystemAuthCredentialStore {
    fn load(&self) -> AuthLoadResult {
        AuthStorage {
            secure: &SYSTEM_KEYRING_STORE,
            legacy: &LegacyAuthStore,
        }
        .load()
    }

    fn save(&self, auth: &TwitchAuthState) -> anyhow::Result<Option<String>> {
        AuthStorage {
            secure: &SYSTEM_KEYRING_STORE,
            legacy: &LegacyAuthStore,
        }
        .save(auth)
    }

    fn clear(&self) -> anyhow::Result<()> {
        AuthStorage {
            secure: &SYSTEM_KEYRING_STORE,
            legacy: &LegacyAuthStore,
        }
        .clear()
    }
}

#[cfg(feature = "app")]
struct KeyringAuthStore<'a> {
    service: &'a str,
    account: &'a str,
}

#[cfg(feature = "app")]
const SYSTEM_KEYRING_STORE: KeyringAuthStore<'static> = KeyringAuthStore {
    service: KEYRING_SERVICE,
    account: KEYRING_ACCOUNT,
};

#[cfg(feature = "app")]
impl AuthSecretStore for KeyringAuthStore<'_> {
    fn load_secret(&self) -> anyhow::Result<Option<String>> {
        let entry = self.entry()?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn save_secret(&self, secret: &str) -> anyhow::Result<()> {
        self.entry()?.set_password(secret)?;
        Ok(())
    }

    fn clear_secret(&self) -> anyhow::Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(feature = "app")]
impl KeyringAuthStore<'_> {
    fn entry(&self) -> anyhow::Result<keyring::Entry> {
        Ok(keyring::Entry::new(self.service, self.account)?)
    }
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

            let auth_snapshot = {
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
                auth.clone()
            };
            let storage_warning =
                save_auth_if_current(&state, pending.generation, auth_snapshot).await?;
            ensure_auth_generation_is_current(&state, pending.generation)?;
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
                clear_missing_scope_twitch_auth(&state, &app, &message).await?;
                return Err(message);
            }
            profile
        }
        Err(validate_error) => {
            let token = match refresh_access_token(&client_id, &refresh_token).await {
                Ok(token) => token,
                Err(refresh_error) => {
                    ensure_auth_generation_is_current(&state, generation)?;
                    if is_definitive_auth_failure(&refresh_error) {
                        let message = to_twitch_user_message(anyhow::anyhow!(
                            "{validate_error}; {refresh_error}"
                        ));
                        clear_invalid_twitch_auth(&state, &app, &message).await?;
                        return Err(message);
                    }
                    return Err(retryable_auth_error_message(&refresh_error));
                }
            };
            let profile = match validate_access_token(&token.access_token).await {
                Ok(validate) => {
                    let profile = TwitchUserProfile::from(validate);
                    if let Err(error) = ensure_required_twitch_scopes(&profile.scopes) {
                        let message = error.to_string();
                        ensure_auth_generation_is_current(&state, generation)?;
                        clear_missing_scope_twitch_auth(&state, &app, &message).await?;
                        return Err(message);
                    }
                    profile
                }
                Err(error) => {
                    ensure_auth_generation_is_current(&state, generation)?;
                    if is_definitive_auth_failure(&error) {
                        let message = to_twitch_user_message(error);
                        clear_invalid_twitch_auth(&state, &app, &message).await?;
                        return Err(message);
                    }
                    return Err(retryable_auth_error_message(&error));
                }
            };
            let auth_snapshot = {
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
                auth.clone()
            };
            let storage_warning = save_auth_if_current(&state, generation, auth_snapshot).await?;
            ensure_auth_generation_is_current(&state, generation)?;
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

    let auth_snapshot = {
        let mut auth = state
            .twitch_auth
            .lock()
            .map_err(|error| error.to_string())?;
        apply_validated_profile(&mut auth, generation, profile.clone())?;
        auth.clone()
    };
    let storage_warning = save_auth_if_current(&state, generation, auth_snapshot).await?;
    ensure_auth_generation_is_current(&state, generation)?;
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
fn apply_validated_profile(
    auth: &mut TwitchAuthState,
    generation: u64,
    profile: TwitchUserProfile,
) -> Result<(), String> {
    if auth.generation != generation {
        return Err(
            "新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。".to_string(),
        );
    }
    auth.profile = Some(profile);
    Ok(())
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
) -> Result<(), crate::settings::validation::ValidationError> {
    let requested_channel = if let Some(channel) = channel_login {
        channel
    } else {
        let settings = state.settings.lock().map_err(|error| error.to_string())?;
        settings.twitch.channel_login.clone()
    };
    let channel_login =
        crate::settings::validation::TwitchLogin::parse(&requested_channel, true)?.into_string();
    connect_validated_channel(channel_login, state, app)
        .await
        .map_err(Into::into)
}

#[cfg(feature = "app")]
async fn connect_validated_channel(
    channel_login: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
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
        clear_missing_scope_twitch_auth(&state, &app, &message).await?;
        return Err(message);
    }

    let channel_login = if channel_login.is_empty() {
        crate::settings::validation::TwitchLogin::parse(&own_login, false)
            .map_err(|error| error.to_string())?
            .into_string()
    } else {
        channel_login
    };
    let generation = NEXT_TWITCH_CONNECTION_GENERATION.fetch_add(1, Ordering::Relaxed);
    let channel_for_log = channel_login.clone();
    let app_for_task = app.clone();
    let task = tokio::spawn(async move {
        let broadcaster = match fetch_twitch_user(&client_id, &access_token, &channel_login).await {
            Ok(broadcaster) => broadcaster,
            Err(error) => {
                let message = to_twitch_user_message(error);
                emit_twitch_chat_status(
                    &app_for_task,
                    TwitchStatus::Error,
                    Some(message.clone()),
                    generation,
                    None,
                );
                emit_app_log(&app_for_task, AppLogLevel::Error, message);
                return;
            }
        };
        let is_current = app_for_task
            .state::<AppState>()
            .twitch_connection
            .lock()
            .ok()
            .and_then(|connection| {
                connection
                    .as_ref()
                    .map(|handle| handle.generation == generation)
            })
            .unwrap_or(false);
        if !is_current {
            return;
        }
        let params = EventSubConnectionParams {
            generation,
            broadcaster_user_id: broadcaster.id,
            broadcaster_login: broadcaster.login,
            user_id,
        };
        run_eventsub_connection(app_for_task, params).await;
    });
    let mut connection = state
        .twitch_connection
        .lock()
        .map_err(|error| error.to_string())?;
    if let Some(handle) = connection.take() {
        handle.abort();
    }
    *connection = Some(TwitchConnectionHandle::new(generation, task));
    emit_twitch_chat_status(
        &app,
        TwitchStatus::Connecting,
        Some(format!(
            "Twitch チャンネル {} に接続しています。",
            channel_for_log
        )),
        generation,
        None,
    );
    emit_app_log(
        &app,
        AppLogLevel::Info,
        format!(
            "Twitch チャンネル {} への EventSub 接続を開始しました。",
            channel_for_log
        ),
    );
    Ok(())
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn twitch_disconnect(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    clear_twitch_auth_state(&state).await?;
    let connection_generation = NEXT_TWITCH_CONNECTION_GENERATION.fetch_add(1, Ordering::Relaxed);
    emit_twitch_chat_status(
        &app,
        TwitchStatus::Disconnected,
        Some("Twitch チャット受信を停止しました。".to_string()),
        connection_generation,
        None,
    );
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
async fn clear_twitch_auth_state(state: &tauri::State<'_, AppState>) -> Result<(), String> {
    clear_twitch_auth_state_with_store(state.twitch_auth.clone(), &state.twitch_auth_store).await?;

    if let Some(handle) = state
        .twitch_connection
        .lock()
        .map_err(|error| error.to_string())?
        .take()
    {
        handle.abort();
    }
    Ok(())
}

/// Clears durable Twitch credentials after first invalidating the in-memory
/// generation. This is kept separate from the Tauri command wiring so the same
/// production path can be exercised with a delayed credential-store backend.
#[cfg(feature = "app")]
async fn clear_twitch_auth_state_with_store(
    auth_state: std::sync::Arc<std::sync::Mutex<TwitchAuthState>>,
    store: &TwitchAuthStore,
) -> Result<(), String> {
    // Invalidate the in-memory generation before waiting for the store. This
    // prevents a concurrent save from being accepted after logout begins.
    let (previous_auth, generation) = {
        let mut auth = auth_state.lock().map_err(|error| error.to_string())?;
        let previous_auth = auth.clone();
        let generation = auth.invalidate_operations();
        auth.token = None;
        auth.profile = None;
        (previous_auth, generation)
    };

    match store.clear_if_current(auth_state.clone(), generation).await {
        Ok(AuthClearOutcome::Cleared) => Ok(()),
        Ok(AuthClearOutcome::Stale | AuthClearOutcome::StaleAfterClear) => {
            Err("新しい Twitch 認証操作が開始されたため、古い解除結果を破棄しました。".to_string())
        }
        Err(error) => {
            let mut auth = auth_state.lock().map_err(|error| error.to_string())?;
            restore_auth_after_failed_clear_if_current(&mut auth, generation, previous_auth);
            Err(to_secure_store_user_message(error))
        }
    }
}

#[cfg(feature = "app")]
fn restore_auth_after_failed_clear_if_current(
    auth: &mut TwitchAuthState,
    generation: u64,
    mut previous_auth: TwitchAuthState,
) {
    if auth.generation == generation {
        // Restoring a credential after a failed delete must not roll back the
        // generation. Older poll/validate/save operations remain stale even
        // though the user can continue using the prior credential.
        previous_auth.generation = generation;
        previous_auth.pending = None;
        *auth = previous_auth;
    }
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
async fn clear_invalid_twitch_auth(
    state: &tauri::State<'_, AppState>,
    app: &tauri::AppHandle<tauri::Wry>,
    error_message: &str,
) -> Result<(), String> {
    clear_twitch_auth_state(state).await?;
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
async fn clear_missing_scope_twitch_auth(
    state: &tauri::State<'_, AppState>,
    app: &tauri::AppHandle<tauri::Wry>,
    error_message: &str,
) -> Result<(), String> {
    clear_twitch_auth_state(state).await?;
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

    let connection_generation = NEXT_TWITCH_CONNECTION_GENERATION.fetch_add(1, Ordering::Relaxed);
    emit_twitch_chat_status(
        &app,
        TwitchStatus::Disconnected,
        Some(if stopped {
            "Twitch チャット受信を停止しました。".to_string()
        } else {
            "Twitch チャット受信は開始されていません。".to_string()
        }),
        connection_generation,
        None,
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

fn twitch_http_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(TWITCH_HTTP_TIMEOUT)
        .timeout(TWITCH_HTTP_TIMEOUT)
        .build()
        .map_err(anyhow::Error::from)
}

async fn request_device_code(client_id: &str) -> anyhow::Result<DeviceCodeResponse> {
    let response = twitch_http_client()?
        .post(TWITCH_DEVICE_URL)
        .form(&[("client_id", client_id), ("scopes", CHAT_READ_SCOPE)])
        .send()
        .await?;

    Ok(parse_json_response(response).await?)
}

async fn poll_device_token(pending: &PendingDeviceAuth) -> Result<TokenResponse, PollAuthError> {
    let response = twitch_http_client()
        .map_err(PollAuthError::Other)?
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

    let response = twitch_http_client()?
        .post(TWITCH_TOKEN_URL)
        .form(&[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await?;

    Ok(parse_json_response(response).await?)
}

async fn validate_access_token(access_token: &str) -> anyhow::Result<ValidateResponse> {
    let response = twitch_http_client()?
        .get(TWITCH_VALIDATE_URL)
        .bearer_auth(access_token)
        .send()
        .await?;

    Ok(parse_json_response(response).await?)
}

async fn fetch_twitch_user(
    client_id: &str,
    access_token: &str,
    login: &str,
) -> anyhow::Result<HelixUser> {
    let response = twitch_http_client()?
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
trait EventSubRuntime: Sync {
    type Socket: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin
        + Send;
    fn connect(
        &self,
        url: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<Self::Socket>> + Send;
    fn subscribe(
        &self,
        params: &EventSubConnectionParams,
        session_id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
    fn status(&self, domain: TwitchStatusDomain, status: TwitchStatus, message: Option<String>);
    fn chat_status(&self, status: TwitchStatus, message: Option<String>, generation: u64);
    fn connected(&self, params: &EventSubConnectionParams, message: String);
    fn log(&self, level: AppLogLevel, message: impl Into<String>);
    fn chat(&self, message: ChatMessage, connection_generation: u64);
}

#[cfg(feature = "app")]
impl EventSubRuntime for tauri::AppHandle<tauri::Wry> {
    type Socket = EventSubSocket;
    async fn connect(&self, url: &str) -> anyhow::Result<Self::Socket> {
        Ok(connect_async(url).await?.0)
    }
    async fn subscribe(
        &self,
        params: &EventSubConnectionParams,
        session_id: &str,
    ) -> anyhow::Result<()> {
        create_chat_message_subscription(self, params, session_id).await
    }
    fn status(&self, domain: TwitchStatusDomain, status: TwitchStatus, message: Option<String>) {
        emit_twitch_status(self, domain, status, message);
    }
    fn chat_status(&self, status: TwitchStatus, message: Option<String>, generation: u64) {
        emit_twitch_chat_status(self, status, message, generation, None);
    }
    fn connected(&self, params: &EventSubConnectionParams, message: String) {
        emit_twitch_chat_status(
            self,
            TwitchStatus::Connected,
            Some(message),
            params.generation,
            Some(TwitchActiveConnection {
                generation: params.generation,
                broadcaster_user_id: params.broadcaster_user_id.clone(),
                broadcaster_login: params.broadcaster_login.clone(),
            }),
        );
    }
    fn log(&self, level: AppLogLevel, message: impl Into<String>) {
        emit_app_log(self, level, message);
    }
    fn chat(&self, message: ChatMessage, connection_generation: u64) {
        emit_twitch_chat_message(self, message.clone(), connection_generation);
        if let Err(error) = enqueue_chat_message_for_speech(self.clone(), message) {
            emit_app_log(self, AppLogLevel::Error, error);
        }
    }
}

#[cfg(feature = "app")]
async fn run_eventsub_connection(
    app: tauri::AppHandle<tauri::Wry>,
    params: EventSubConnectionParams,
) {
    run_eventsub_connection_with(&app, &params).await;
}

#[cfg(feature = "app")]
async fn run_eventsub_connection_with<R: EventSubRuntime>(
    app: &R,
    params: &EventSubConnectionParams,
) {
    let mut reconnect_backoff = EventSubReconnectBackoff::default();
    let mut seen_message_ids = MessageDedupe::new(DEDUPE_CACHE_LIMIT, DEDUPE_CACHE_TTL);

    loop {
        if let Err(error) = run_eventsub_session(
            app,
            params,
            TWITCH_EVENTSUB_WS_URL,
            &mut reconnect_backoff,
            &mut seen_message_ids,
        )
        .await
        {
            if let Some(terminal) = error.downcast_ref::<EventSubTerminalError>() {
                // Terminal API and revocation failures have already emitted their
                // actionable UI status. Never turn configuration/auth failures into
                // an infinite reconnect loop.
                app.log(AppLogLevel::Error, terminal.to_string());
                break;
            }
            let wait_seconds = reconnect_backoff.next_delay_after_failure();
            let message = format!(
                "Twitch EventSub が切断されました。{} 秒後に再接続します: {error}",
                wait_seconds
            );
            app.chat_status(
                TwitchStatus::Reconnecting,
                Some(message.clone()),
                params.generation,
            );
            app.log(AppLogLevel::Warning, message);
            tokio::time::sleep(Duration::from_secs(wait_seconds)).await;
        }
    }
}

#[cfg(feature = "app")]
async fn run_eventsub_session<R: EventSubRuntime>(
    app: &R,
    params: &EventSubConnectionParams,
    url: &str,
    reconnect_backoff: &mut EventSubReconnectBackoff,
    seen_message_ids: &mut MessageDedupe,
) -> anyhow::Result<()> {
    app.chat_status(
        TwitchStatus::Connecting,
        Some(format!(
            "Twitch チャンネル {} に接続しています。",
            params.broadcaster_login
        )),
        params.generation,
    );

    let mut socket = tokio::time::timeout(TWITCH_WS_HANDSHAKE_TIMEOUT, app.connect(url))
        .await
        .map_err(|_| {
            anyhow::anyhow!("Twitch EventSub WebSocket の接続が期限内に完了しませんでした。")
        })??;
    let mut keepalive_timeout = Duration::from_secs(40);
    let mut keepalive_deadline =
        tokio::time::Instant::now() + keepalive_timeout + Duration::from_secs(5);

    loop {
        let next_message = tokio::time::timeout_at(keepalive_deadline, socket.next())
            .await
            .map_err(|_| anyhow::anyhow!("Twitch から keepalive または通知が届きませんでした。"))?
            .ok_or_else(|| anyhow::anyhow!("Twitch EventSub WebSocket が閉じられました。"))??;

        match process_eventsub_frame(
            app,
            &mut socket,
            next_message,
            seen_message_ids,
            Utc::now(),
            params.generation,
        )
        .await?
        {
            EventSubFrameAction::Continue => {}
            EventSubFrameAction::Activity => {
                keepalive_deadline =
                    tokio::time::Instant::now() + keepalive_timeout + Duration::from_secs(5);
            }
            EventSubFrameAction::Welcome(session) => {
                keepalive_timeout =
                    complete_eventsub_welcome(app, params, session, true, reconnect_backoff)
                        .await?;
                keepalive_deadline =
                    tokio::time::Instant::now() + keepalive_timeout + Duration::from_secs(5);
            }
            EventSubFrameAction::Reconnect(reconnect_url) => {
                reconnect_backoff.record_handover_started();
                app.chat_status(
                    TwitchStatus::Reconnecting,
                    Some("Twitch から再接続要求を受け取りました。".to_string()),
                    params.generation,
                );
                app.log(
                    AppLogLevel::Warning,
                    "Twitch EventSub の再接続要求を受け取りました。旧接続を維持して切り替えます。",
                );

                let (new_socket, session) = handover_eventsub_session(
                    app,
                    &mut socket,
                    reconnect_url,
                    seen_message_ids,
                    params.generation,
                )
                .await?;
                socket = new_socket;
                keepalive_timeout =
                    complete_eventsub_welcome(app, params, session, false, reconnect_backoff)
                        .await?;
                keepalive_deadline =
                    tokio::time::Instant::now() + keepalive_timeout + Duration::from_secs(5);
            }
        }
    }
}

#[cfg(feature = "app")]
async fn complete_eventsub_welcome<R: EventSubRuntime>(
    app: &R,
    params: &EventSubConnectionParams,
    session: EventSubSession,
    subscribe_on_welcome: bool,
    reconnect_backoff: &mut EventSubReconnectBackoff,
) -> anyhow::Result<Duration> {
    let keepalive_timeout = session
        .keepalive_timeout_seconds
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(40));

    if subscribe_on_welcome {
        app.subscribe(params, &session.id).await?;
    }

    reconnect_backoff.record_session_established();
    app.connected(
        params,
        format!(
            "Twitch チャンネル {} に接続しました。",
            params.broadcaster_login
        ),
    );
    app.log(
        AppLogLevel::Info,
        format!("Twitch EventSub session {} を開始しました。", session.id),
    );

    Ok(keepalive_timeout)
}

#[cfg(feature = "app")]
async fn handover_eventsub_session<R: EventSubRuntime>(
    app: &R,
    old_socket: &mut R::Socket,
    reconnect_url: String,
    seen_message_ids: &mut MessageDedupe,
    connection_generation: u64,
) -> anyhow::Result<(R::Socket, EventSubSession)> {
    let deadline = tokio::time::Instant::now() + EVENTSUB_RECONNECT_HANDOVER_TIMEOUT;
    let mut connect = Box::pin(async {
        tokio::time::timeout(TWITCH_WS_HANDSHAKE_TIMEOUT, app.connect(&reconnect_url))
            .await
            .map_err(|_| {
                anyhow::anyhow!("Twitch EventSub の再接続が期限内に完了しませんでした。")
            })?
    });
    let mut reconnect_socket: Option<R::Socket> = None;
    let mut reconnect_error: Option<anyhow::Error> = None;

    loop {
        if let Some(new_socket) = reconnect_socket.as_mut() {
            tokio::select! {
                biased;
                _ = tokio::time::sleep_until(deadline) => break,
                old_message = old_socket.next() => {
                    let old_message = old_message
                        .ok_or_else(|| anyhow::anyhow!("新しい welcome 前に旧 EventSub WebSocket が閉じられました。"))??;
                    match process_eventsub_frame(app, old_socket, old_message, seen_message_ids, Utc::now(), connection_generation).await? {
                        EventSubFrameAction::Continue | EventSubFrameAction::Activity | EventSubFrameAction::Welcome(_) => {}
                        EventSubFrameAction::Reconnect(_) => {
                            app.log( AppLogLevel::Warning, "Twitch EventSub の再接続要求を重複受信しました。切り替えを継続します。");
                        }
                    }
                }
                new_message = new_socket.next() => {
                    let new_message = new_message
                        .ok_or_else(|| anyhow::anyhow!("新しい EventSub WebSocket が welcome 前に閉じられました。"));
                    match new_message {
                        Ok(Ok(new_message)) => match process_eventsub_frame(app, new_socket, new_message, seen_message_ids, Utc::now(), connection_generation).await {
                            Ok(EventSubFrameAction::Welcome(session)) => {
                                // The old stream may become ready while the new welcome is polled.
                                // Drain already available frames before replacing it; never wait for
                                // a future old frame or exceed the handover deadline.
                                while tokio::time::Instant::now() < deadline {
                                    match old_socket.next().now_or_never() {
                                        Some(Some(Ok(Message::Close(_)))) | Some(Some(Err(_))) | Some(None) | None => break,
                                        Some(Some(Ok(frame))) => {
                                            process_eventsub_frame(app, old_socket, frame, seen_message_ids, Utc::now(), connection_generation).await?;
                                        }
                                    }
                                }
                                return Ok((reconnect_socket.take().expect("new socket exists"), session));
                            }
                            Ok(EventSubFrameAction::Continue | EventSubFrameAction::Activity) => {}
                            Ok(EventSubFrameAction::Reconnect(_)) => {
                                reconnect_error = Some(anyhow::anyhow!("新しい EventSub WebSocket が welcome 前に再接続要求を返しました。"));
                                reconnect_socket = None;
                            }
                            Err(error) => {
                                reconnect_error = Some(error);
                                reconnect_socket = None;
                            }
                        },
                        Ok(Err(error)) => {
                            reconnect_error = Some(error.into());
                            reconnect_socket = None;
                        }
                        Err(error) => {
                            reconnect_error = Some(error);
                            reconnect_socket = None;
                        }
                    }
                }
            }
        } else if reconnect_error.is_none() {
            tokio::select! {
                biased;
                _ = tokio::time::sleep_until(deadline) => break,
                connection = connect.as_mut() => match connection {
                    Ok(socket) => reconnect_socket = Some(socket),
                    Err(error) => reconnect_error = Some(error),
                },
                old_message = old_socket.next() => {
                    let old_message = old_message
                        .ok_or_else(|| anyhow::anyhow!("新しい welcome 前に旧 EventSub WebSocket が閉じられました。"))??;
                    match process_eventsub_frame(app, old_socket, old_message, seen_message_ids, Utc::now(), connection_generation).await? {
                        EventSubFrameAction::Continue | EventSubFrameAction::Activity | EventSubFrameAction::Welcome(_) => {}
                        EventSubFrameAction::Reconnect(_) => {
                            app.log( AppLogLevel::Warning, "Twitch EventSub の再接続要求を重複受信しました。切り替えを継続します。");
                        }
                    }
                }
            }
        } else {
            tokio::select! {
                biased;
                _ = tokio::time::sleep_until(deadline) => break,
                old_message = old_socket.next() => {
                    let old_message = old_message
                        .ok_or_else(|| anyhow::anyhow!("新しい welcome 前に旧 EventSub WebSocket が閉じられました。"))??;
                    match process_eventsub_frame(app, old_socket, old_message, seen_message_ids, Utc::now(), connection_generation).await? {
                        EventSubFrameAction::Continue | EventSubFrameAction::Activity | EventSubFrameAction::Welcome(_) => {}
                        EventSubFrameAction::Reconnect(_) => {
                            app.log( AppLogLevel::Warning, "Twitch EventSub の再接続要求を重複受信しました。切り替えを継続します。");
                        }
                    }
                }
            }
        }
    }

    let detail = reconnect_error
        .map(|error| error.to_string())
        .unwrap_or_else(|| "新しい welcome が期限内に届きませんでした。".to_string());
    Err(anyhow::anyhow!(
        "Twitch EventSub の切り替えに失敗しました。旧接続を {} 秒維持した後、通常再接続へ戻ります: {detail}",
        EVENTSUB_RECONNECT_HANDOVER_TIMEOUT.as_secs()
    ))
}

#[cfg(feature = "app")]
async fn process_eventsub_frame<R: EventSubRuntime>(
    app: &R,
    socket: &mut R::Socket,
    next_message: Message,
    seen_message_ids: &mut MessageDedupe,
    frame_received_at: DateTime<Utc>,
    connection_generation: u64,
) -> anyhow::Result<EventSubFrameAction> {
    match next_message {
        Message::Text(text) => {
            let envelope = serde_json::from_str::<EventSubEnvelope>(&text)?;
            match envelope.metadata.message_type.as_str() {
                "session_welcome" => {
                    let session = envelope.payload.session.ok_or_else(|| {
                        anyhow::anyhow!("Twitch の welcome に session がありません。")
                    })?;
                    Ok(EventSubFrameAction::Welcome(session))
                }
                "session_keepalive" => Ok(EventSubFrameAction::Activity),
                "session_reconnect" => {
                    let reconnect_url = envelope
                        .payload
                        .session
                        .and_then(|session| session.reconnect_url)
                        .ok_or_else(|| {
                            anyhow::anyhow!("Twitch の reconnect に reconnect_url がありません。")
                        })?;
                    Ok(EventSubFrameAction::Reconnect(reconnect_url))
                }
                "notification" => {
                    if let Some(normalized) = normalize_chat_message(envelope, frame_received_at)? {
                        if let Some(warning) = normalized.timestamp_warning {
                            app.log(AppLogLevel::Warning, warning);
                        }
                        let message = normalized.message;
                        let dedupe_id = message.id.clone();
                        if seen_message_ids.insert(dedupe_id) {
                            app.chat(message, connection_generation);
                        }
                    }
                    Ok(EventSubFrameAction::Activity)
                }
                "revocation" => {
                    let subscription = envelope.payload.subscription;
                    let reason = subscription
                        .as_ref()
                        .map(|item| format!("{} ({})", item.status, item.kind))
                        .unwrap_or_else(|| "理由不明".to_string());
                    let terminal = match subscription.as_ref().map(|item| item.status.as_str()) {
                        Some("authorization_revoked") => {
                            let message = format!("Twitch EventSub 購読の認可が取り消されました。Login から再ログインしてください: {reason}");
                            app.chat_status(
                                TwitchStatus::AuthRequired,
                                None,
                                connection_generation,
                            );
                            app.status(
                                TwitchStatusDomain::Auth,
                                TwitchStatus::AuthRequired,
                                Some(message.clone()),
                            );
                            EventSubTerminalError::AuthRequired { message }
                        }
                        Some("user_removed") => {
                            let message = format!("Twitch EventSub の対象ユーザーが存在しません。接続チャンネルを確認してください: {reason}");
                            app.chat_status(
                                TwitchStatus::Error,
                                Some(message.clone()),
                                connection_generation,
                            );
                            EventSubTerminalError::Permanent { message }
                        }
                        Some("version_removed") => {
                            let message = format!("Twitch EventSub の購読バージョンが廃止されました。アプリを更新してください: {reason}");
                            app.chat_status(
                                TwitchStatus::Error,
                                Some(message.clone()),
                                connection_generation,
                            );
                            EventSubTerminalError::Permanent { message }
                        }
                        _ => {
                            let message = format!("Twitch EventSub 購読が取り消されました。再接続せず停止します: {reason}");
                            app.chat_status(
                                TwitchStatus::Error,
                                Some(message.clone()),
                                connection_generation,
                            );
                            EventSubTerminalError::Permanent { message }
                        }
                    };
                    Err(anyhow::Error::new(terminal))
                }
                _ => Ok(EventSubFrameAction::Continue),
            }
        }
        Message::Ping(payload) => {
            socket.send(Message::Pong(payload)).await?;
            Ok(EventSubFrameAction::Continue)
        }
        Message::Close(frame) => Err(anyhow::anyhow!(
            "Twitch EventSub WebSocket が閉じられました: {:?}",
            frame
        )),
        _ => Ok(EventSubFrameAction::Continue),
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
        .map_err(|error| SubscriptionRequestError::Retryable(anyhow::anyhow!(error.to_string())))?
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
            let message = "Twitch 認証を更新しても EventSub 購読が拒否されました。Login から再ログインしてください。";
            clear_eventsub_auth(app, message).await?;
            Err(anyhow::Error::new(EventSubTerminalError::AuthRequired {
                message: message.to_string(),
            }))
        }
        Err(SubscriptionRequestError::AuthRequired(message)) => {
            Err(anyhow::Error::new(EventSubTerminalError::AuthRequired {
                message,
            }))
        }
        Err(SubscriptionRequestError::Permanent(error)) => {
            let message = subscription_error_user_message(&error);
            if matches!(error, TwitchApiError::Http { status: 403, .. }) {
                clear_eventsub_auth(app, &message).await?;
                Err(anyhow::Error::new(EventSubTerminalError::AuthRequired {
                    message,
                }))
            } else {
                Err(anyhow::Error::new(EventSubTerminalError::Permanent {
                    message,
                }))
            }
        }
        Err(SubscriptionRequestError::Retryable(error)) => Err(error),
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
    RefreshFuture: std::future::Future<Output = Result<String, SubscriptionRequestError>>,
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
) -> Result<String, SubscriptionRequestError> {
    let latest_credentials = app
        .state::<AppState>()
        .twitch_auth
        .lock()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .eventsub_credentials()
        .map_err(SubscriptionRequestError::Retryable)?;
    if latest_credentials.refresh_token != credentials.refresh_token {
        return Ok(latest_credentials.access_token);
    }

    let (refreshed, refreshed_profile) =
        match refresh_and_validate(&TwitchOAuthHttp, credentials).await {
            Ok(result) => result,
            Err(error) => return Err(classify_eventsub_refresh_error(app, error).await),
        };
    if let Err(error) = ensure_required_twitch_scopes(&refreshed_profile.scopes) {
        let message = error.to_string();
        if let Some(access_token) = clear_eventsub_auth_for_missing_scope_if_current(
            app,
            &credentials.refresh_token,
            &message,
        )
        .await
        .map_err(SubscriptionRequestError::Retryable)?
        {
            // Another EventSub re-subscription refreshed and rotated the credentials
            // while this request was validating its now-stale refresh result.  Its
            // access token is authoritative, so leave that newer authentication in
            // place and retry the subscription with it.
            return Ok(access_token);
        }
        return Err(SubscriptionRequestError::AuthRequired(message));
    }

    let state = app.state::<AppState>();
    let (access_token, did_refresh, storage_warning) = persist_eventsub_rotation(
        state.twitch_auth.clone(),
        &state.twitch_auth_store,
        credentials,
        refreshed,
        refreshed_profile,
    )
    .await?;

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
trait OAuthTransport {
    fn refresh(
        &self,
        client_id: &str,
        refresh_token: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<TokenResponse>> + Send;
    fn validate(
        &self,
        access_token: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<ValidateResponse>> + Send;
}

#[cfg(feature = "app")]
struct TwitchOAuthHttp;
#[cfg(feature = "app")]
impl OAuthTransport for TwitchOAuthHttp {
    async fn refresh(&self, client_id: &str, refresh_token: &str) -> anyhow::Result<TokenResponse> {
        refresh_access_token(client_id, refresh_token).await
    }
    async fn validate(&self, access_token: &str) -> anyhow::Result<ValidateResponse> {
        validate_access_token(access_token).await
    }
}

#[cfg(feature = "app")]
async fn refresh_and_validate(
    http: &(impl OAuthTransport + Sync),
    credentials: &EventSubAuthCredentials,
) -> anyhow::Result<(TokenResponse, TwitchUserProfile)> {
    let token = http
        .refresh(&credentials.client_id, &credentials.refresh_token)
        .await?;
    let profile = http.validate(&token.access_token).await?.into();
    Ok((token, profile))
}

#[cfg(feature = "app")]
async fn persist_eventsub_rotation(
    auth_state: std::sync::Arc<std::sync::Mutex<TwitchAuthState>>,
    store: &TwitchAuthStore,
    credentials: &EventSubAuthCredentials,
    refreshed: TokenResponse,
    profile: TwitchUserProfile,
) -> Result<(String, bool, Option<String>), SubscriptionRequestError> {
    let (access_token, snapshot, generation) = {
        let mut auth = auth_state
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let current = auth.eventsub_credentials()?;
        if current.refresh_token != credentials.refresh_token {
            return Ok((current.access_token, false, None));
        }
        let access = auth.replace_token(refreshed, profile)?;
        (access, auth.clone(), auth.generation)
    };
    let warning = match store
        .save_if_current(auth_state.clone(), generation, snapshot)
        .await?
    {
        AuthSaveOutcome::Saved(warning) => warning,
        AuthSaveOutcome::Stale => {
            return Err(SubscriptionRequestError::Retryable(anyhow::anyhow!(
                "新しい Twitch 認証操作が開始されたため、古い保存結果を破棄しました。"
            )))
        }
    };
    if auth_state
        .lock()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .generation
        != generation
    {
        return Err(SubscriptionRequestError::Retryable(anyhow::anyhow!(
            "古い Twitch 認証更新を破棄しました。"
        )));
    }
    Ok((access_token, true, warning))
}

/// Classifies OAuth refresh and validation failures before they reach the EventSub
/// reconnect supervisor. Only timeouts and HTTP 5xx errors may request a retry;
/// malformed or forbidden requests are terminal and cannot create a reconnect loop.
#[cfg(feature = "app")]
async fn classify_eventsub_refresh_error(
    app: &tauri::AppHandle<tauri::Wry>,
    error: anyhow::Error,
) -> SubscriptionRequestError {
    let api_error = match error.downcast::<TwitchApiError>() {
        Ok(api_error) => api_error,
        Err(error) => return SubscriptionRequestError::Retryable(error),
    };

    if api_error.auth_failure().is_some() {
        let message = api_error.user_message();
        return match clear_eventsub_auth(app, &message).await {
            Ok(()) => SubscriptionRequestError::AuthRequired(message),
            Err(error) => SubscriptionRequestError::Retryable(error),
        };
    }
    if api_error.is_transient() {
        return SubscriptionRequestError::Retryable(anyhow::Error::new(api_error));
    }
    SubscriptionRequestError::Permanent(api_error)
}

#[cfg(feature = "app")]
async fn clear_eventsub_auth(
    app: &tauri::AppHandle<tauri::Wry>,
    error_message: &str,
) -> anyhow::Result<()> {
    let state = app.state::<AppState>();
    clear_invalid_twitch_auth(&state, app, error_message)
        .await
        .map_err(|error| anyhow::anyhow!(error))
}

/// Clears an EventSub authentication only when it still belongs to the refresh
/// request that found a missing required scope.  Concurrent EventSub retries can
/// rotate a refresh token while an older request is awaiting `/validate`; clearing
/// unconditionally would discard the newer, valid authentication.
///
/// Returns the newer access token when the credentials have already rotated.
#[cfg(feature = "app")]
async fn clear_eventsub_auth_for_missing_scope_if_current(
    app: &tauri::AppHandle<tauri::Wry>,
    expected_refresh_token: &str,
    error_message: &str,
) -> anyhow::Result<Option<String>> {
    let state = app.state::<AppState>();
    let generation = {
        // Compare and invalidate under the short auth lock, then perform
        // credential I/O after it has been released.
        let mut auth = state
            .twitch_auth
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        if let Some(access_token) =
            clear_auth_for_eventsub_missing_scope_if_current(&mut auth, expected_refresh_token)?
        {
            return Ok(Some(access_token));
        }
        auth.generation
    };

    match state
        .twitch_auth_store
        .clear_if_current(state.twitch_auth.clone(), generation)
        .await
        .map_err(to_secure_store_user_message)
        .map_err(|error| anyhow::anyhow!(error))?
    {
        AuthClearOutcome::Cleared => {}
        AuthClearOutcome::Stale | AuthClearOutcome::StaleAfterClear => {
            let auth = state
                .twitch_auth
                .lock()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            return Ok(Some(auth.eventsub_credentials()?.access_token));
        }
    }
    if let Some(handle) = state
        .twitch_connection
        .lock()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .take()
    {
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

    auth.generation = auth.generation.wrapping_add(1);
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
    let response = twitch_http_client()
        .map_err(SubscriptionRequestError::Retryable)?
        .post(TWITCH_EVENTSUB_SUBSCRIPTIONS_URL)
        .header("Client-Id", client_id)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await
        .map_err(TwitchApiError::from);

    let response = match response {
        Ok(response) => response,
        Err(error) if error.is_transient() => {
            return Err(SubscriptionRequestError::Retryable(anyhow::Error::new(
                error,
            )));
        }
        Err(error) => return Err(SubscriptionRequestError::Permanent(error)),
    };

    match parse_json_response::<serde_json::Value>(response).await {
        Ok(_) => Ok(()),
        Err(TwitchApiError::Http { status: 401, .. }) => {
            Err(SubscriptionRequestError::Unauthorized)
        }
        Err(error) if error.is_transient() => Err(SubscriptionRequestError::Retryable(
            anyhow::Error::new(error),
        )),
        Err(error) => Err(SubscriptionRequestError::Permanent(error)),
    }
}

fn retry_backoff_seconds(attempt: u64) -> u64 {
    match attempt {
        0 | 1 => 2,
        2 => 5,
        3 => 10,
        _ => 30,
    }
}

async fn parse_json_response<T>(response: reqwest::Response) -> Result<T, TwitchApiError>
where
    T: for<'de> Deserialize<'de>,
{
    if response.status().is_success() {
        return Ok(response.json::<T>().await?);
    }

    let status = response.status().as_u16();
    let error = response.json::<OAuthErrorResponse>().await.ok();
    let code = error.as_ref().and_then(|item| item.error.clone());
    let message = error
        .and_then(|item| item.message.or(item.error))
        .unwrap_or_else(|| format!("HTTP {status}"));

    Err(TwitchApiError::Http {
        status,
        code,
        message,
    })
}

#[cfg(feature = "app")]
async fn save_auth_if_current(
    state: &tauri::State<'_, AppState>,
    generation: u64,
    auth: TwitchAuthState,
) -> Result<Option<String>, String> {
    match state
        .twitch_auth_store
        .save_if_current(state.twitch_auth.clone(), generation, auth)
        .await
        .map_err(to_secure_store_user_message)?
    {
        AuthSaveOutcome::Saved(warning) => Ok(warning),
        AuthSaveOutcome::Stale => {
            Err("新しい Twitch 認証操作が開始されたため、古い保存結果を破棄しました。".to_string())
        }
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

#[cfg(all(test, feature = "app"))]
mod test_harness;

#[cfg(all(test, feature = "app", target_os = "windows"))]
mod windows_store_tests;

#[cfg(test)]
mod tests;
