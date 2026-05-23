#[cfg(feature = "app")]
use crate::app_events::{
    emit_app_log, emit_twitch_chat_message, emit_twitch_status, AppLogLevel, TwitchStatus,
};
#[cfg(feature = "app")]
use crate::settings::{default_twitch_client_id, AppState};
use chrono::Utc;
#[cfg(feature = "app")]
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
#[cfg(feature = "app")]
use std::time::Duration;
#[cfg(all(feature = "app", target_os = "linux"))]
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
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
const CHAT_READ_SCOPE: &str = "user:read:chat";
const KEYRING_SERVICE: &str = "rice.twitch.oauth";
const KEYRING_ACCOUNT: &str = "default";
const CHANNEL_CHAT_MESSAGE_TYPE: &str = "channel.chat.message";
const CHANNEL_CHAT_MESSAGE_VERSION: &str = "1";
const DEDUPE_CACHE_LIMIT: usize = 500;
#[cfg(all(feature = "app", target_os = "linux"))]
const FALLBACK_AUTH_DIR: &str = ".rice";
#[cfg(all(feature = "app", target_os = "linux"))]
const FALLBACK_AUTH_FILE: &str = "twitch-auth.json";

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
    pub received_at: String,
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
    pending: Option<PendingDeviceAuth>,
    token: Option<TwitchToken>,
    profile: Option<TwitchUserProfile>,
}

#[derive(Debug, Clone)]
struct PendingDeviceAuth {
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
    message_timestamp: String,
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
    client_id: String,
    access_token: String,
    broadcaster_user_id: String,
    broadcaster_login: String,
    user_id: String,
}

#[cfg(feature = "app")]
enum EventSubSessionExit {
    Reconnect(String),
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
    fn profile(&self) -> Option<TwitchUserProfile> {
        self.profile.clone()
    }

    fn restore(stored: StoredTwitchAuth) -> Self {
        let mut profile = stored.profile;
        if profile.client_id.trim().is_empty() {
            profile.client_id = stored.client_id.clone();
        }
        Self {
            pending: None,
            token: Some(TwitchToken {
                access_token: stored.access_token,
                refresh_token: stored.refresh_token,
                scopes: stored.scopes,
                expires_in: stored.expires_in,
            }),
            profile: Some(profile),
        }
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
}

#[cfg(feature = "app")]
pub struct TwitchAuthStore;

#[cfg(feature = "app")]
impl TwitchAuthStore {
    pub fn load() -> anyhow::Result<Option<TwitchAuthState>> {
        let keyring_result = match keyring_entry() {
            Ok(entry) => match entry.get_password() {
                Ok(secret) => serde_json::from_str::<StoredTwitchAuth>(&secret)
                    .map(TwitchAuthState::restore)
                    .map(Some)
                    .map_err(anyhow::Error::from),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(error) => Err(error.into()),
            },
            Err(error) => Err(error),
        };

        match keyring_result {
            Ok(Some(auth)) => Ok(Some(auth)),
            Ok(None) => load_fallback_auth(),
            Err(error) => match load_fallback_auth() {
                Ok(Some(auth)) => Ok(Some(auth)),
                Ok(None) => Err(error),
                Err(fallback_error) => Err(anyhow::anyhow!("{error}; {fallback_error}")),
            },
        }
    }

    fn save(auth: &TwitchAuthState) -> anyhow::Result<Option<String>> {
        let stored = auth
            .stored_auth()
            .ok_or_else(|| anyhow::anyhow!("保存できる Twitch 認証状態がありません。"))?;
        let secret = serde_json::to_string(&stored)?;

        match keyring_entry()
            .and_then(|entry| entry.set_password(&secret).map_err(anyhow::Error::from))
        {
            Ok(()) => {
                clear_fallback_auth()?;
                Ok(None)
            }
            Err(error) => save_fallback_auth(&stored, error),
        }
    }

    fn clear() -> anyhow::Result<()> {
        let keyring_result = keyring_entry().and_then(|entry| match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        });

        let fallback_result = clear_fallback_auth();
        match (keyring_result, fallback_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => handle_keyring_clear_error(error),
            (Ok(()), Err(error)) => Err(error),
            (Err(keyring_error), Err(fallback_error)) => {
                Err(anyhow::anyhow!("{keyring_error}; {fallback_error}"))
            }
        }
    }
}

#[cfg(feature = "app")]
fn keyring_entry() -> anyhow::Result<keyring::Entry> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)?)
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn load_fallback_auth() -> anyhow::Result<Option<TwitchAuthState>> {
    let path = match fallback_auth_path() {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };
    if !path.exists() {
        return Ok(None);
    }

    ensure_fallback_permissions(&path)?;
    let secret = fs::read_to_string(path)?;
    let stored = serde_json::from_str::<StoredTwitchAuth>(&secret)?;
    Ok(Some(TwitchAuthState::restore(stored)))
}

#[cfg(all(feature = "app", not(target_os = "linux")))]
fn load_fallback_auth() -> anyhow::Result<Option<TwitchAuthState>> {
    Ok(None)
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn save_fallback_auth(
    stored: &StoredTwitchAuth,
    keyring_error: anyhow::Error,
) -> anyhow::Result<Option<String>> {
    let path = fallback_auth_path()?;
    ensure_fallback_parent(&path)?;

    let temp_path = path.with_extension("json.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temp_path)?;
    file.write_all(serde_json::to_string_pretty(stored)?.as_bytes())?;
    file.sync_all()?;
    drop(file);

    fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600))?;
    fs::rename(&temp_path, &path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;

    Ok(Some(to_local_file_store_user_message(keyring_error, &path)))
}

#[cfg(all(feature = "app", not(target_os = "linux")))]
fn save_fallback_auth(
    _stored: &StoredTwitchAuth,
    keyring_error: anyhow::Error,
) -> anyhow::Result<Option<String>> {
    Err(keyring_error)
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn clear_fallback_auth() -> anyhow::Result<()> {
    let path = match fallback_auth_path() {
        Ok(path) => path,
        Err(_) => return Ok(()),
    };
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(all(feature = "app", not(target_os = "linux")))]
fn clear_fallback_auth() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn handle_keyring_clear_error(_error: anyhow::Error) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(all(feature = "app", not(target_os = "linux")))]
fn handle_keyring_clear_error(error: anyhow::Error) -> anyhow::Result<()> {
    Err(error)
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn fallback_auth_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        anyhow::anyhow!("HOME が設定されていないため、Twitch 認証情報を保存できません。")
    })?;
    Ok(PathBuf::from(home)
        .join(FALLBACK_AUTH_DIR)
        .join(FALLBACK_AUTH_FILE))
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn ensure_fallback_parent(path: &Path) -> anyhow::Result<()> {
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
fn ensure_fallback_permissions(path: &Path) -> anyhow::Result<()> {
    ensure_fallback_parent(path)?;
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

    let response = request_device_code(&client_id)
        .await
        .map_err(to_twitch_user_message)?;
    let auth_start = TwitchDeviceAuthStart {
        user_code: response.user_code.clone(),
        verification_uri: response.verification_uri,
        expires_in: response.expires_in,
        interval: response.interval,
    };

    let mut auth = state
        .twitch_auth
        .lock()
        .map_err(|error| error.to_string())?;
    auth.pending = Some(PendingDeviceAuth {
        client_id,
        device_code: response.device_code,
        interval: response.interval,
    });
    auth.token = None;
    auth.profile = None;
    emit_twitch_status(
        &app,
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
        let auth = state
            .twitch_auth
            .lock()
            .map_err(|error| error.to_string())?;
        auth.pending.clone()
    }
    .ok_or_else(|| "Twitch 認証が開始されていません。".to_string())?;

    match poll_device_token(&pending).await {
        Ok(token) => {
            let profile = validate_access_token(&token.access_token)
                .await
                .map_err(to_twitch_user_message)?;
            let profile = TwitchUserProfile::from(profile);

            {
                let mut auth = state
                    .twitch_auth
                    .lock()
                    .map_err(|error| error.to_string())?;
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
                return Ok(TwitchAuthPollResult::Authorized {
                    profile,
                    storage_warning,
                });
            }
        }
        Err(PollAuthError::Pending) => Ok(TwitchAuthPollResult::Pending {
            message: {
                emit_twitch_status(
                    &app,
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
            if let Some(stored) = &mut auth.pending {
                stored.interval = interval;
            }
            emit_twitch_status(
                &app,
                TwitchStatus::Connecting,
                Some("Twitch 認証の確認間隔を延長しました。".to_string()),
            );
            Ok(TwitchAuthPollResult::SlowDown {
                message: "確認間隔が短すぎます。少し待ってから再確認してください。".to_string(),
                interval,
            })
        }
        Err(PollAuthError::Denied) => Ok(TwitchAuthPollResult::Denied {
            message: {
                emit_twitch_status(
                    &app,
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
        }),
        Err(PollAuthError::Expired) => Ok(TwitchAuthPollResult::Expired {
            message: {
                emit_twitch_status(
                    &app,
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
        }),
        Err(PollAuthError::Other(error)) => {
            let message = to_twitch_user_message(error);
            emit_twitch_status(&app, TwitchStatus::Error, Some(message.clone()));
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
    let (access_token, refresh_token, client_id) = {
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
            token.access_token.clone(),
            token.refresh_token.clone(),
            client_id,
        )
    };

    let profile = match validate_access_token(&access_token).await {
        Ok(validate) => TwitchUserProfile::from(validate),
        Err(validate_error) => {
            let token = refresh_access_token(&client_id, &refresh_token)
                .await
                .map_err(|refresh_error| {
                    to_twitch_user_message(anyhow::anyhow!("{validate_error}; {refresh_error}"))
                })?;
            let profile = TwitchUserProfile::from(
                validate_access_token(&token.access_token)
                    .await
                    .map_err(to_twitch_user_message)?,
            );
            let mut auth = state
                .twitch_auth
                .lock()
                .map_err(|error| error.to_string())?;
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
    auth.profile = Some(profile.clone());
    let storage_warning = save_or_storage_warning(&auth);
    emit_twitch_status(
        &app,
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

    let (access_token, client_id, user_id, own_login) = {
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
        )
    };

    let channel_login = if channel_login.is_empty() {
        own_login
    } else {
        channel_login
    };
    let broadcaster = fetch_twitch_user(&client_id, &access_token, &channel_login)
        .await
        .map_err(to_twitch_user_message)?;
    let params = EventSubConnectionParams {
        client_id,
        access_token,
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
    auth.pending = None;
    auth.token = None;
    auth.profile = None;
    TwitchAuthStore::clear().map_err(to_secure_store_user_message)?;
    emit_twitch_status(
        &app,
        TwitchStatus::Disconnected,
        Some("Twitch 連携を解除しました。".to_string()),
    );
    emit_app_log(&app, AppLogLevel::Info, "Twitch 連携を解除しました。");
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
        TwitchStatus::Disconnected,
        Some(if stopped {
            "Twitch コメント受信を停止しました。".to_string()
        } else {
            "Twitch コメント受信は開始されていません。".to_string()
        }),
    );
    emit_app_log(
        &app,
        AppLogLevel::Info,
        if stopped {
            "Twitch コメント受信を停止しました。"
        } else {
            "Twitch コメント受信は開始されていません。"
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
    match error.error.as_deref() {
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
    let mut retry_attempt = 0u64;

    loop {
        match run_eventsub_session(&app, &params, &url, subscribe_on_welcome).await {
            Ok(EventSubSessionExit::Reconnect(reconnect_url)) => {
                retry_attempt = 0;
                url = reconnect_url;
                subscribe_on_welcome = false;
                emit_twitch_status(
                    &app,
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
                    emit_app_log(&app, AppLogLevel::Error, error_message);
                    break;
                }
                retry_attempt = retry_attempt.saturating_add(1);
                let wait_seconds = retry_backoff_seconds(retry_attempt);
                let message = format!(
                    "Twitch EventSub が切断されました。{} 秒後に再接続します: {error}",
                    wait_seconds
                );
                emit_twitch_status(&app, TwitchStatus::Reconnecting, Some(message.clone()));
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
) -> anyhow::Result<EventSubSessionExit> {
    emit_twitch_status(
        app,
        TwitchStatus::Connecting,
        Some(format!(
            "Twitch チャンネル {} に接続しています。",
            params.broadcaster_login
        )),
    );

    let (mut socket, _) = connect_async(url).await?;
    let mut keepalive_timeout = Duration::from_secs(40);
    let mut seen_message_ids = MessageDedupe::new(DEDUPE_CACHE_LIMIT);

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
                            create_chat_message_subscription(params, &session.id).await?;
                        }
                        emit_twitch_status(
                            app,
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
                        if let Some(message) = normalize_chat_message(envelope)? {
                            let dedupe_id = message.id.clone();
                            if seen_message_ids.insert(dedupe_id) {
                                emit_twitch_chat_message(app, message);
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
                            TwitchStatus::AuthRequired,
                            Some(format!(
                                "Twitch EventSub 購読が取り消されました。再ログインしてください: {reason}"
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
    params: &EventSubConnectionParams,
    session_id: &str,
) -> anyhow::Result<()> {
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
        .header("Client-Id", &params.client_id)
        .bearer_auth(&params.access_token)
        .json(&body)
        .send()
        .await?;

    parse_json_response::<serde_json::Value>(response).await?;
    Ok(())
}

fn normalize_chat_message(envelope: EventSubEnvelope) -> anyhow::Result<Option<ChatMessage>> {
    if envelope.metadata.subscription_type.as_deref() != Some(CHANNEL_CHAT_MESSAGE_TYPE) {
        return Ok(None);
    }

    let event = match envelope.payload.event {
        Some(event) => event,
        None => return Ok(None),
    };
    let event = serde_json::from_value::<EventSubChatMessageEvent>(event)?;
    let received_at = if envelope.metadata.message_timestamp.is_empty() {
        Utc::now().to_rfc3339()
    } else {
        envelope.metadata.message_timestamp
    };
    let id = if event.message_id.is_empty() {
        envelope.metadata.message_id
    } else {
        event.message_id
    };

    Ok(Some(ChatMessage {
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
    seen: HashSet<String>,
    order: VecDeque<String>,
}

impl MessageDedupe {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            seen: HashSet::new(),
            order: VecDeque::new(),
        }
    }

    fn insert(&mut self, id: String) -> bool {
        if !self.seen.insert(id.clone()) {
            return false;
        }

        self.order.push_back(id);
        while self.order.len() > self.limit {
            if let Some(old_id) = self.order.pop_front() {
                self.seen.remove(&old_id);
            }
        }
        true
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
    #[cfg(target_os = "linux")]
    {
        return format!(
            "Twitch 認証情報を保存できませんでした。Linux では Secret Service 対応の資格情報ストア（GNOME Keyring、KWallet、KeePassXC Secret Service など）または ~/.rice/twitch-auth.json へのローカル保存を使います: {error}"
        );
    }

    #[cfg(not(target_os = "linux"))]
    format!("Twitch 認証情報を安全に保存できませんでした。ログインは続行しましたが、アプリ再起動後は再ログインが必要です。OS の資格情報ストアを確認してください: {error}")
}

#[cfg(all(feature = "app", target_os = "linux"))]
fn to_local_file_store_user_message(error: anyhow::Error, path: &Path) -> String {
    format!(
        "OS の資格情報ストアに保存できなかったため、Twitch 認証情報を {} に保存しました。ディレクトリは 700、ファイルは 600 で作成していますが、暗号化はされません: {error}",
        path.display()
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

enum PollAuthError {
    Pending,
    SlowDown,
    Denied,
    Expired,
    Other(anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::{normalize_chat_message, EventSubEnvelope, MessageDedupe};

    #[test]
    fn parses_channel_chat_message_fixture() {
        let fixture = include_str!("fixtures/channel_chat_message.json");
        let envelope = serde_json::from_str::<EventSubEnvelope>(fixture).unwrap();
        let message = normalize_chat_message(envelope).unwrap().unwrap();

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
        assert_eq!(message.received_at, "2023-11-06T18:11:47.492253549Z");
    }

    #[test]
    fn message_dedupe_rejects_duplicate_ids() {
        let mut dedupe = MessageDedupe::new(2);

        assert!(dedupe.insert("a".to_string()));
        assert!(!dedupe.insert("a".to_string()));
        assert!(dedupe.insert("b".to_string()));
        assert!(dedupe.insert("c".to_string()));
        assert!(dedupe.insert("a".to_string()));
    }
}
