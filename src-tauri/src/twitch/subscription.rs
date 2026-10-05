//! Twitch subscription responsibility boundary.
use super::auth_service::{clear_invalid_twitch_auth, AuthRuntime};
use super::auth_state::{
    ensure_required_twitch_scopes, EventSubAuthCredentials, EventSubConnectionParams,
    TokenResponse, TwitchAuthState, TwitchUserProfile,
};
use super::auth_store::{AuthClearOutcome, AuthSaveOutcome, TwitchAuthStore};
use super::error::{
    subscription_error_user_message, to_secure_store_user_message, EventSubTerminalError,
    SubscriptionRequestError, TwitchApiError,
};
use super::oauth::{parse_json_response, refresh_and_validate, twitch_http_client};
use super::{
    CHANNEL_CHAT_MESSAGE_TYPE, CHANNEL_CHAT_MESSAGE_VERSION, TWITCH_EVENTSUB_SUBSCRIPTIONS_URL,
};
use crate::app_events::{AppLogLevel, TwitchAuthRequiredReason};

pub(super) trait SubscriptionRuntime: AuthRuntime {
    fn send_subscription(
        &self,
        params: &EventSubConnectionParams,
        session_id: &str,
        client_id: &str,
        access_token: &str,
    ) -> impl std::future::Future<Output = Result<(), SubscriptionRequestError>> + Send;
}

pub(super) async fn create_chat_message_subscription(
    app: &impl SubscriptionRuntime,
    params: &EventSubConnectionParams,
    session_id: &str,
) -> anyhow::Result<()> {
    let credentials = app
        .auth()
        .lock()
        .map_err(|error| SubscriptionRequestError::Retryable(anyhow::anyhow!(error.to_string())))?
        .eventsub_credentials()?;
    let subscription_client_id = credentials.client_id.clone();
    let refresh_app = app;
    let refresh_credentials = credentials.clone();

    match retry_eventsub_subscription(
        credentials.access_token,
        |access_token| {
            let client_id = subscription_client_id.clone();
            async move {
                app.send_subscription(params, session_id, &client_id, &access_token)
                    .await
            }
        },
        move || {
            let app = refresh_app;
            let credentials = refresh_credentials.clone();
            async move { refresh_eventsub_access_token(app, &credentials).await }
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

pub(super) async fn retry_eventsub_subscription<
    Subscribe,
    SubscribeFuture,
    Refresh,
    RefreshFuture,
>(
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

pub(super) async fn refresh_eventsub_access_token(
    app: &impl SubscriptionRuntime,
    credentials: &EventSubAuthCredentials,
) -> Result<String, SubscriptionRequestError> {
    let latest_credentials = app
        .auth()
        .lock()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .eventsub_credentials()
        .map_err(SubscriptionRequestError::Retryable)?;
    if latest_credentials.refresh_token != credentials.refresh_token {
        return Ok(latest_credentials.access_token);
    }

    let (refreshed, refreshed_profile) = match refresh_and_validate(app, credentials).await {
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

    let state = app;
    let (access_token, did_refresh, storage_warning) = persist_eventsub_rotation(
        state.auth().clone(),
        state.store(),
        credentials,
        refreshed,
        refreshed_profile,
    )
    .await?;

    if did_refresh {
        app.auth_log(
            AppLogLevel::Info,
            "Twitch EventSub の再購読前に認証を更新しました。",
        );
    }
    if let Some(warning) = storage_warning {
        app.auth_log(AppLogLevel::Warning, warning);
    }
    Ok(access_token)
}

pub(super) async fn persist_eventsub_rotation(
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
pub(super) async fn classify_eventsub_refresh_error(
    app: &impl SubscriptionRuntime,
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

pub(super) async fn clear_eventsub_auth(
    app: &impl SubscriptionRuntime,
    error_message: &str,
) -> anyhow::Result<()> {
    let state = app;
    clear_invalid_twitch_auth(state, error_message)
        .await
        .map_err(|error| anyhow::anyhow!(error))
}

/// Clears an EventSub authentication only when it still belongs to the refresh
/// request that found a missing required scope.  Concurrent EventSub retries can
/// rotate a refresh token while an older request is awaiting `/validate`; clearing
/// unconditionally would discard the newer, valid authentication.
///
/// Returns the newer access token when the credentials have already rotated.
pub(super) async fn clear_eventsub_auth_for_missing_scope_if_current(
    app: &impl SubscriptionRuntime,
    expected_refresh_token: &str,
    error_message: &str,
) -> anyhow::Result<Option<String>> {
    let state = app;
    let generation = {
        // Compare and invalidate under the short auth lock, then perform
        // credential I/O after it has been released.
        let mut auth = state
            .auth()
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
        .store()
        .clear_if_current(state.auth().clone(), generation)
        .await
        .map_err(to_secure_store_user_message)
        .map_err(|error| anyhow::anyhow!(error))?
    {
        AuthClearOutcome::Cleared => {}
        AuthClearOutcome::Stale | AuthClearOutcome::StaleAfterClear => {
            let auth = state
                .auth()
                .lock()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            return Ok(Some(auth.eventsub_credentials()?.access_token));
        }
    }
    state.cancel_chat().map_err(anyhow::Error::msg)?;
    app.require_auth(
        TwitchAuthRequiredReason::MissingRequiredScope,
        error_message,
    );
    app.auth_log(AppLogLevel::Warning, error_message);
    Ok(None)
}

pub(super) fn clear_auth_for_eventsub_missing_scope_if_current(
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

pub(super) async fn send_chat_message_subscription(
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
