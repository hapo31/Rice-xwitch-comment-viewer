//! Production adapter: Tauri state/events and external transports stay here.
use super::auth_service::AuthRuntime;
use super::auth_state::{
    DeviceCodeResponse, EventSubConnectionParams, HelixUser, PendingDeviceAuth, TokenResponse,
    TwitchAuthState, ValidateResponse,
};
use super::auth_store::TwitchAuthStore;
use super::chat_delivery::dispatch_chat_message;
use super::chat_service::{ChatRuntime, TwitchConnectionHandle};
use super::error::SubscriptionRequestError;
use super::eventsub::{EventSubRuntime, EventSubSocket};
use super::model::ChatMessage;
use super::oauth::{
    fetch_twitch_user, DeviceOAuthTransport, OAuthTransport, PollAuthError, TwitchOAuthHttp,
};
use super::subscription::{
    create_chat_message_subscription, send_chat_message_subscription, SubscriptionRuntime,
};
use crate::app_events::{
    emit_app_log, emit_twitch_auth_required, emit_twitch_chat_message, emit_twitch_chat_status,
    emit_twitch_status, AppLogLevel, TwitchActiveConnection, TwitchAuthRequiredReason,
    TwitchStatus, TwitchStatusDomain,
};
use crate::settings::{default_twitch_client_id, AppState};
use crate::speech::enqueue_chat_message_for_speech;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::Manager;
use tokio_tungstenite::connect_async;

static NEXT_TWITCH_CONNECTION_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub(super) struct TauriTwitchRuntime {
    app: tauri::AppHandle<tauri::Wry>,
    auth: std::sync::Arc<std::sync::Mutex<TwitchAuthState>>,
    store: TwitchAuthStore,
}
impl TauriTwitchRuntime {
    pub(super) fn new(state: &AppState, app: tauri::AppHandle<tauri::Wry>) -> Self {
        Self {
            app,
            auth: state.twitch_auth.clone(),
            store: state.twitch_auth_store.clone(),
        }
    }
}
impl OAuthTransport for TauriTwitchRuntime {
    async fn refresh(&self, client_id: &str, refresh_token: &str) -> anyhow::Result<TokenResponse> {
        TwitchOAuthHttp.refresh(client_id, refresh_token).await
    }
    async fn validate(&self, access_token: &str) -> anyhow::Result<ValidateResponse> {
        TwitchOAuthHttp.validate(access_token).await
    }
}
impl DeviceOAuthTransport for TauriTwitchRuntime {
    async fn device_code(&self, client_id: &str) -> anyhow::Result<DeviceCodeResponse> {
        TwitchOAuthHttp.device_code(client_id).await
    }
    async fn poll_token(
        &self,
        pending: &PendingDeviceAuth,
    ) -> Result<TokenResponse, PollAuthError> {
        TwitchOAuthHttp.poll_token(pending).await
    }
}
impl AuthRuntime for TauriTwitchRuntime {
    fn auth(&self) -> &std::sync::Arc<std::sync::Mutex<TwitchAuthState>> {
        &self.auth
    }
    fn store(&self) -> &TwitchAuthStore {
        &self.store
    }
    fn client_id(&self) -> String {
        default_twitch_client_id()
    }
    fn now(&self) -> std::time::SystemTime {
        std::time::SystemTime::now()
    }
    fn cancel_chat(&self) -> Result<bool, String> {
        let state = self.app.state::<AppState>();
        let stopped = state
            .twitch_connection
            .lock()
            .map_err(|error| error.to_string())?
            .take();
        if let Some(handle) = stopped {
            handle.abort();
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn auth_status(
        &self,
        domain: TwitchStatusDomain,
        status: TwitchStatus,
        message: Option<String>,
    ) {
        emit_twitch_status(&self.app, domain, status, message);
    }
    fn auth_log(&self, level: AppLogLevel, message: impl Into<String>) {
        emit_app_log(&self.app, level, message);
    }
    fn require_auth(&self, reason: TwitchAuthRequiredReason, message: impl Into<String>) {
        emit_twitch_auth_required(&self.app, reason, message);
    }
}
impl SubscriptionRuntime for TauriTwitchRuntime {
    async fn send_subscription(
        &self,
        params: &EventSubConnectionParams,
        session_id: &str,
        client_id: &str,
        access_token: &str,
    ) -> Result<(), SubscriptionRequestError> {
        send_chat_message_subscription(params, session_id, client_id, access_token).await
    }
}
impl ChatRuntime for TauriTwitchRuntime {
    fn preferred_channel(&self) -> Result<String, String> {
        let state = self.app.state::<AppState>();
        let settings = state.settings.lock().map_err(|error| error.to_string())?;
        Ok(settings.twitch.channel_login.clone())
    }
    fn next_generation(&self) -> u64 {
        NEXT_TWITCH_CONNECTION_GENERATION.fetch_add(1, Ordering::Relaxed)
    }
    fn replace_connection(&self, connection: TwitchConnectionHandle) -> Result<(), String> {
        let state = self.app.state::<AppState>();
        let mut current = state
            .twitch_connection
            .lock()
            .map_err(|error| error.to_string())?;
        if let Some(previous) = current.take() {
            previous.abort();
        }
        *current = Some(connection);
        Ok(())
    }
    fn connection_is_current(&self, generation: u64) -> bool {
        self.app
            .state::<AppState>()
            .twitch_connection
            .lock()
            .ok()
            .and_then(|current| {
                current
                    .as_ref()
                    .map(|handle| handle.generation == generation)
            })
            .unwrap_or(false)
    }
    async fn lookup_user(
        &self,
        client_id: &str,
        access_token: &str,
        login: &str,
    ) -> anyhow::Result<HelixUser> {
        fetch_twitch_user(client_id, access_token, login).await
    }
}
impl EventSubRuntime for TauriTwitchRuntime {
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
        emit_twitch_status(&self.app, domain, status, message);
    }
    fn chat_status(&self, status: TwitchStatus, message: Option<String>, generation: u64) {
        emit_twitch_chat_status(&self.app, status, message, generation, None);
    }
    fn connected(&self, params: &EventSubConnectionParams, message: String) {
        emit_twitch_chat_status(
            &self.app,
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
        emit_app_log(&self.app, level, message);
    }
    fn chat(&self, message: ChatMessage) {
        let state = self.app.state::<AppState>();
        let current = match state.twitch_connection.lock() {
            Ok(current) => current,
            Err(error) => {
                emit_app_log(&self.app, AppLogLevel::Error, error.to_string());
                return;
            }
        };
        let active_generation = current.as_ref().map(|connection| connection.generation);
        // Keep stop/replacement behind this shared delivery boundary so UI and
        // speech observe the same accepted model before its generation expires.
        dispatch_chat_message(
            &message,
            active_generation,
            |message| emit_twitch_chat_message(&self.app, message.clone()),
            |message| {
                if let Err(error) =
                    enqueue_chat_message_for_speech(self.app.clone(), message.clone())
                {
                    emit_app_log(&self.app, AppLogLevel::Error, error);
                }
            },
        );
    }
}
