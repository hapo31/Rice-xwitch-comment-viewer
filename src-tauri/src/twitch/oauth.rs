//! Twitch oauth responsibility boundary.
use super::auth_state::{
    DeviceCodeResponse, HelixUser, HelixUsersResponse, OAuthErrorResponse, PendingDeviceAuth,
    TokenResponse, ValidateResponse,
};
#[cfg(feature = "app")]
use super::auth_state::{EventSubAuthCredentials, TwitchUserProfile};
use super::error::TwitchApiError;
use super::http::TwitchHttp;
use super::CHAT_READ_SCOPE;
use serde::Deserialize;

impl TwitchHttp {
    pub(super) async fn request_device_code(
        &self,
        client_id: &str,
    ) -> anyhow::Result<DeviceCodeResponse> {
        let response = self
            .client
            .post(&self.endpoints.device)
            .form(&[("client_id", client_id), ("scopes", CHAT_READ_SCOPE)])
            .send()
            .await?;

        Ok(parse_json_response(response).await?)
    }

    pub(super) async fn poll_device_token(
        &self,
        pending: &PendingDeviceAuth,
    ) -> Result<TokenResponse, PollAuthError> {
        let response = self
            .client
            .post(&self.endpoints.token)
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

    pub(super) async fn refresh_access_token(
        &self,
        client_id: &str,
        refresh_token: &str,
    ) -> anyhow::Result<TokenResponse> {
        if client_id.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "Twitch Client ID が見つかりません。再ログインしてください。"
            ));
        }

        let response = self
            .client
            .post(&self.endpoints.token)
            .form(&[
                ("client_id", client_id),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await?;

        Ok(parse_json_response(response).await?)
    }

    pub(super) async fn validate_access_token(
        &self,
        access_token: &str,
    ) -> anyhow::Result<ValidateResponse> {
        let response = self
            .client
            .get(&self.endpoints.validate)
            .bearer_auth(access_token)
            .send()
            .await?;

        Ok(parse_json_response(response).await?)
    }

    pub(super) async fn fetch_twitch_user(
        &self,
        client_id: &str,
        access_token: &str,
        login: &str,
    ) -> anyhow::Result<HelixUser> {
        let response = self
            .client
            .get(&self.endpoints.users)
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
}

pub(super) fn oauth_error_code(error: &OAuthErrorResponse) -> Option<&str> {
    error.error.as_deref().or(error.message.as_deref())
}

#[cfg(feature = "app")]
pub(super) trait OAuthTransport {
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
impl OAuthTransport for TwitchHttp {
    async fn refresh(&self, client_id: &str, refresh_token: &str) -> anyhow::Result<TokenResponse> {
        self.refresh_access_token(client_id, refresh_token).await
    }
    async fn validate(&self, access_token: &str) -> anyhow::Result<ValidateResponse> {
        self.validate_access_token(access_token).await
    }
}

#[cfg(feature = "app")]
pub(super) async fn refresh_and_validate(
    http: &(impl OAuthTransport + Sync),
    credentials: &EventSubAuthCredentials,
) -> anyhow::Result<(TokenResponse, TwitchUserProfile)> {
    let token = http
        .refresh(&credentials.client_id, &credentials.refresh_token)
        .await?;
    let profile = http.validate(&token.access_token).await?.into();
    Ok((token, profile))
}

pub(super) async fn parse_json_response<T>(response: reqwest::Response) -> Result<T, TwitchApiError>
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

pub(super) enum PollAuthError {
    Pending,
    SlowDown,
    Denied,
    Expired,
    Other(anyhow::Error),
}

#[cfg(feature = "app")]
pub(super) trait DeviceOAuthTransport: OAuthTransport {
    fn device_code(
        &self,
        client_id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<DeviceCodeResponse>> + Send;
    fn poll_token(
        &self,
        pending: &PendingDeviceAuth,
    ) -> impl std::future::Future<Output = Result<TokenResponse, PollAuthError>> + Send;
}
#[cfg(feature = "app")]
impl DeviceOAuthTransport for TwitchHttp {
    async fn device_code(&self, client_id: &str) -> anyhow::Result<DeviceCodeResponse> {
        self.request_device_code(client_id).await
    }
    async fn poll_token(
        &self,
        pending: &PendingDeviceAuth,
    ) -> Result<TokenResponse, PollAuthError> {
        self.poll_device_token(pending).await
    }
}
