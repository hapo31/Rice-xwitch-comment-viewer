#[cfg(feature = "app")]
use crate::settings::AppState;
use serde::{Deserialize, Serialize};
#[cfg(all(feature = "app", target_os = "linux"))]
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const TWITCH_DEVICE_URL: &str = "https://id.twitch.tv/oauth2/device";
const TWITCH_TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
const TWITCH_VALIDATE_URL: &str = "https://id.twitch.tv/oauth2/validate";
const CHAT_READ_SCOPE: &str = "user:read:chat";
const KEYRING_SERVICE: &str = "rice.twitch.oauth";
const KEYRING_ACCOUNT: &str = "default";
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    Twitch,
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
struct OAuthErrorResponse {
    message: Option<String>,
    error: Option<String>,
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
        Self {
            pending: None,
            token: Some(TwitchToken {
                access_token: stored.access_token,
                refresh_token: stored.refresh_token,
                scopes: stored.scopes,
                expires_in: stored.expires_in,
            }),
            profile: Some(stored.profile),
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
) -> Result<TwitchDeviceAuthStart, String> {
    let client_id = {
        let settings = state.settings.lock().map_err(|error| error.to_string())?;
        settings.twitch.client_id.trim().to_string()
    };

    if client_id.is_empty() {
        return Err("Twitch Client ID を設定してから認証を開始してください。".to_string());
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

    Ok(auth_start)
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn twitch_poll_auth(
    state: tauri::State<'_, AppState>,
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
                return Ok(TwitchAuthPollResult::Authorized {
                    profile,
                    storage_warning,
                });
            }
        }
        Err(PollAuthError::Pending) => Ok(TwitchAuthPollResult::Pending {
            message: "Twitch の認可完了を待っています。ブラウザでコードを入力してください。"
                .to_string(),
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
            Ok(TwitchAuthPollResult::SlowDown {
                message: "確認間隔が短すぎます。少し待ってから再確認してください。".to_string(),
                interval,
            })
        }
        Err(PollAuthError::Denied) => Ok(TwitchAuthPollResult::Denied {
            message: "Twitch 認証がキャンセルされました。必要なら再度開始してください。"
                .to_string(),
        }),
        Err(PollAuthError::Expired) => Ok(TwitchAuthPollResult::Expired {
            message: "Twitch 認証コードの期限が切れました。再度開始してください。".to_string(),
        }),
        Err(PollAuthError::Other(error)) => Err(to_twitch_user_message(error)),
    }
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn twitch_validate_auth(
    state: tauri::State<'_, AppState>,
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
            .or_else(|| {
                state
                    .settings
                    .lock()
                    .ok()
                    .map(|settings| settings.twitch.client_id.clone())
            })
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
pub fn twitch_disconnect(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut auth = state
        .twitch_auth
        .lock()
        .map_err(|error| error.to_string())?;
    auth.pending = None;
    auth.token = None;
    auth.profile = None;
    TwitchAuthStore::clear().map_err(to_secure_store_user_message)?;
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
