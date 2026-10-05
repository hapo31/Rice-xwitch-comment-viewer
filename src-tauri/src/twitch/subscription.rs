//! Twitch subscription responsibility boundary.
use super::auth_service::{
    clear_credentials_if_current, refresh_credentials_if_current, stale_credential_message,
    AuthRuntime, MissingTwitchScope, StaleCredentialResponse,
};
#[cfg(test)]
use super::auth_state::TwitchAuthState;
use super::auth_state::{EventSubAuthCredentials, EventSubConnectionParams};
use super::error::{
    subscription_error_user_message, EventSubTerminalError, SubscriptionRequestError,
    TwitchApiError,
};
use super::oauth::{parse_json_response, twitch_http_client};
use super::{
    CHANNEL_CHAT_MESSAGE_TYPE, CHANNEL_CHAT_MESSAGE_VERSION, TWITCH_EVENTSUB_SUBSCRIPTIONS_URL,
};
use crate::app_events::AppLogLevel;

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
    let (credentials, auth_user_id) = {
        let auth = app.auth().lock().map_err(|error| {
            SubscriptionRequestError::Retryable(anyhow::anyhow!(error.to_string()))
        })?;
        let credentials = auth.eventsub_credentials()?;
        let user_id = auth
            .profile
            .as_ref()
            .map(|profile| profile.user_id.clone())
            .ok_or_else(|| {
                SubscriptionRequestError::Retryable(anyhow::anyhow!(
                    "Twitch のユーザー情報がありません。認証を確認してください。"
                ))
            })?;
        (credentials, user_id)
    };
    if credentials.generation != params.auth_generation
        || credentials.client_id != params.client_id
        || auth_user_id != params.user_id
    {
        return Err(anyhow::Error::new(
            EventSubTerminalError::ObsoleteConnection,
        ));
    }
    let subscription_client_id = credentials.client_id.clone();
    let refresh_app = app;
    let refresh_credentials = credentials.clone();
    let used_credentials = std::sync::Arc::new(std::sync::Mutex::new(Some(credentials.clone())));
    let refresh_used_credentials = used_credentials.clone();

    match retry_eventsub_subscription(
        credentials.access_token.clone(),
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
            let used_credentials = refresh_used_credentials.clone();
            async move {
                let (access_token, request_credentials) =
                    refresh_eventsub_access_token(app, &credentials).await?;
                *used_credentials.lock().map_err(|error| {
                    SubscriptionRequestError::Retryable(anyhow::anyhow!(error.to_string()))
                })? = Some(request_credentials);
                Ok(access_token)
            }
        },
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(SubscriptionRequestError::Unauthorized) => {
            let message = "Twitch 認証を更新しても EventSub 購読が拒否されました。Login から再ログインしてください。";
            let request_credentials = used_credentials
                .lock()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?
                .clone()
                .expect("the initial subscription attempt always has a credential snapshot");
            let cleared = clear_credentials_if_current(
                app,
                request_credentials.generation,
                request_credentials.credential_revision,
                &request_credentials.access_token,
                &request_credentials.refresh_token,
                message,
                false,
            )
            .await
            .map_err(anyhow::Error::msg)?;
            if !cleared {
                if !auth_session_is_current(app, &request_credentials) {
                    return Err(anyhow::Error::new(
                        EventSubTerminalError::ObsoleteConnection,
                    ));
                }
                return Err(SubscriptionRequestError::Retryable(anyhow::anyhow!(
                    stale_credential_message()
                ))
                .into());
            }
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
                let request_credentials = used_credentials
                    .lock()
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?
                    .clone()
                    .expect("the initial subscription attempt always has a credential snapshot");
                let cleared = clear_credentials_if_current(
                    app,
                    request_credentials.generation,
                    request_credentials.credential_revision,
                    &request_credentials.access_token,
                    &request_credentials.refresh_token,
                    &message,
                    false,
                )
                .await
                .map_err(anyhow::Error::msg)?;
                if !cleared {
                    if !auth_session_is_current(app, &request_credentials) {
                        return Err(anyhow::Error::new(
                            EventSubTerminalError::ObsoleteConnection,
                        ));
                    }
                    return Err(SubscriptionRequestError::Retryable(anyhow::anyhow!(
                        stale_credential_message()
                    ))
                    .into());
                }
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
        Err(SubscriptionRequestError::ObsoleteConnection) => Err(anyhow::Error::new(
            EventSubTerminalError::ObsoleteConnection,
        )),
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
) -> Result<(String, EventSubAuthCredentials), SubscriptionRequestError> {
    let _credential_update = app.store().lock_credential_update().await;
    let latest_credentials = app
        .auth()
        .lock()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .eventsub_credentials()
        .map_err(SubscriptionRequestError::Retryable)?;
    if latest_credentials.generation != credentials.generation
        || latest_credentials.client_id != credentials.client_id
        || latest_credentials.user_id != credentials.user_id
    {
        return Err(SubscriptionRequestError::ObsoleteConnection);
    }
    if latest_credentials.credential_revision != credentials.credential_revision
        || latest_credentials.access_token != credentials.access_token
        || latest_credentials.refresh_token != credentials.refresh_token
    {
        return Ok((latest_credentials.access_token.clone(), latest_credentials));
    }

    let (_profile, storage_warning) = match refresh_credentials_if_current(app, credentials).await {
        Ok(result) => result,
        Err(error) if error.is::<StaleCredentialResponse>() => {
            return latest_eventsub_credentials(app, credentials)
                .map(|latest| (latest.access_token.clone(), latest));
        }
        Err(error) => {
            if let Some(scope_error) = error.downcast_ref::<MissingTwitchScope>() {
                let message = scope_error.0.clone();
                if clear_credentials_if_current(
                    app,
                    credentials.generation,
                    credentials.credential_revision,
                    &credentials.access_token,
                    &credentials.refresh_token,
                    &message,
                    true,
                )
                .await
                .map_err(|error| SubscriptionRequestError::Retryable(anyhow::Error::msg(error)))?
                {
                    return Err(SubscriptionRequestError::AuthRequired(message));
                }
                return latest_eventsub_credentials(app, credentials)
                    .map(|latest| (latest.access_token.clone(), latest));
            }
            return Err(classify_eventsub_refresh_error(app, credentials, error).await);
        }
    };
    let request_credentials = latest_eventsub_credentials(app, credentials)?;

    app.auth_log(
        AppLogLevel::Info,
        "Twitch EventSub の再購読前に認証を更新しました。",
    );
    if let Some(warning) = storage_warning {
        app.auth_log(AppLogLevel::Warning, warning);
    }
    Ok((
        request_credentials.access_token.clone(),
        request_credentials,
    ))
}

fn latest_eventsub_credentials(
    app: &impl SubscriptionRuntime,
    expected: &EventSubAuthCredentials,
) -> Result<EventSubAuthCredentials, SubscriptionRequestError> {
    let auth = app
        .auth()
        .lock()
        .map_err(|error| SubscriptionRequestError::Retryable(anyhow::anyhow!(error.to_string())))?;
    let same_auth_session = auth.generation == expected.generation
        && auth.profile.as_ref().is_some_and(|profile| {
            profile.client_id == expected.client_id && profile.user_id == expected.user_id
        });
    if !same_auth_session {
        return Err(SubscriptionRequestError::ObsoleteConnection);
    }
    auth.eventsub_credentials()
        .map_err(SubscriptionRequestError::Retryable)
}

fn auth_session_is_current(
    app: &impl SubscriptionRuntime,
    expected: &EventSubAuthCredentials,
) -> bool {
    app.auth().lock().is_ok_and(|auth| {
        auth.generation == expected.generation
            && auth.profile.as_ref().is_some_and(|profile| {
                profile.client_id == expected.client_id && profile.user_id == expected.user_id
            })
    })
}

/// Classifies OAuth refresh and validation failures before they reach the EventSub
/// reconnect supervisor. Only timeouts and HTTP 5xx errors may request a retry;
/// malformed or forbidden requests are terminal and cannot create a reconnect loop.
pub(super) async fn classify_eventsub_refresh_error(
    app: &impl SubscriptionRuntime,
    credentials: &EventSubAuthCredentials,
    error: anyhow::Error,
) -> SubscriptionRequestError {
    let api_error = match error.downcast::<TwitchApiError>() {
        Ok(api_error) => api_error,
        Err(error) => return SubscriptionRequestError::Retryable(error),
    };

    if api_error.auth_failure().is_some() {
        let message = api_error.user_message();
        return match clear_credentials_if_current(
            app,
            credentials.generation,
            credentials.credential_revision,
            &credentials.access_token,
            &credentials.refresh_token,
            &message,
            false,
        )
        .await
        {
            Ok(true) => SubscriptionRequestError::AuthRequired(message),
            Ok(false) => {
                SubscriptionRequestError::Retryable(anyhow::anyhow!(stale_credential_message()))
            }
            Err(error) => SubscriptionRequestError::Retryable(anyhow::anyhow!(error)),
        };
    }
    if api_error.is_transient() {
        return SubscriptionRequestError::Retryable(anyhow::Error::new(api_error));
    }
    SubscriptionRequestError::Permanent(api_error)
}

#[cfg(test)]
pub(super) fn clear_auth_for_eventsub_missing_scope_if_current(
    auth: &mut TwitchAuthState,
    expected: &EventSubAuthCredentials,
) -> anyhow::Result<Option<String>> {
    let current_credentials = auth.eventsub_credentials()?;
    if current_credentials.generation != expected.generation
        || current_credentials.credential_revision != expected.credential_revision
        || current_credentials.access_token != expected.access_token
        || current_credentials.refresh_token != expected.refresh_token
    {
        return Ok(Some(current_credentials.access_token));
    }

    auth.generation = auth.generation.wrapping_add(1);
    auth.credential_revision = auth.credential_revision.wrapping_add(1);
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
