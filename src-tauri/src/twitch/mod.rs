#[cfg(feature = "app")]
use crate::settings::AppState;
use serde::{Deserialize, Serialize};

const TWITCH_DEVICE_URL: &str = "https://id.twitch.tv/oauth2/device";
const TWITCH_TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
const TWITCH_VALIDATE_URL: &str = "https://id.twitch.tv/oauth2/validate";
const CHAT_READ_SCOPE: &str = "user:read:chat";

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
    #[allow(dead_code)]
    refresh_token: String,
    #[allow(dead_code)]
    scopes: Vec<String>,
    #[allow(dead_code)]
    expires_in: u64,
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
    Pending { message: String, interval: u64 },
    SlowDown { message: String, interval: u64 },
    Authorized { profile: TwitchUserProfile },
    Denied { message: String },
    Expired { message: String },
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
    scope: Vec<String>,
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

            let mut auth = state
                .twitch_auth
                .lock()
                .map_err(|error| error.to_string())?;
            auth.pending = None;
            auth.profile = Some(profile.clone());
            auth.token = Some(TwitchToken {
                access_token: token.access_token,
                refresh_token: token.refresh_token,
                scopes: token.scope,
                expires_in: token.expires_in,
            });

            Ok(TwitchAuthPollResult::Authorized { profile })
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
) -> Result<TwitchUserProfile, String> {
    let access_token = {
        let auth = state
            .twitch_auth
            .lock()
            .map_err(|error| error.to_string())?;
        auth.token
            .as_ref()
            .map(|token| token.access_token.clone())
            .ok_or_else(|| "Twitch にログインしていません。".to_string())?
    };

    let profile = TwitchUserProfile::from(
        validate_access_token(&access_token)
            .await
            .map_err(to_twitch_user_message)?,
    );

    let mut auth = state
        .twitch_auth
        .lock()
        .map_err(|error| error.to_string())?;
    auth.profile = Some(profile.clone());
    Ok(profile)
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

enum PollAuthError {
    Pending,
    SlowDown,
    Denied,
    Expired,
    Other(anyhow::Error),
}
