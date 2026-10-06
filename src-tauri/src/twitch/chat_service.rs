//! Validated channel selection, connection ownership and chat lifecycle.
use super::auth_service::{clear_missing_scope_twitch_auth, clear_twitch_auth_state, AuthRuntime};
use super::auth_state::{ensure_required_twitch_scopes, EventSubConnectionParams, HelixUser};
use super::error::to_twitch_user_message;
use super::eventsub::{EventSubClient, EventSubRuntime};
use crate::app_events::{AppLogLevel, TwitchStatus, TwitchStatusDomain};

#[derive(Debug)]
pub struct TwitchConnectionHandle {
    pub(super) generation: u64,
    pub(super) task: tokio::task::JoinHandle<()>,
}

impl TwitchConnectionHandle {
    pub(super) fn new(generation: u64, task: tokio::task::JoinHandle<()>) -> Self {
        Self { generation, task }
    }

    pub(super) fn abort(&self) {
        self.task.abort();
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

pub(super) trait ChatRuntime:
    AuthRuntime + EventSubRuntime + Clone + Send + 'static
{
    fn preferred_channel(&self) -> Result<String, String>;
    fn next_generation(&self) -> u64;
    fn replace_connection(&self, connection: TwitchConnectionHandle) -> Result<(), String>;
    fn connection_is_current(&self, generation: u64) -> bool;
    fn lookup_user(
        &self,
        client_id: &str,
        access_token: &str,
        login: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<HelixUser>> + Send;
}
pub(super) struct TwitchChatService<R> {
    runtime: R,
}
impl<R: ChatRuntime> TwitchChatService<R> {
    pub(super) fn new(runtime: R) -> Self {
        Self { runtime }
    }
    pub(super) async fn connect(
        &self,
        channel_login: Option<String>,
    ) -> Result<(), crate::settings::validation::ValidationError> {
        let requested_channel = match channel_login {
            Some(channel) => channel,
            None => self.runtime.preferred_channel()?,
        };
        let channel_login =
            crate::settings::validation::TwitchLogin::parse(&requested_channel, true)?
                .into_string();
        connect_validated_channel(channel_login, &self.runtime)
            .await
            .map_err(Into::into)
    }
    pub(super) async fn disconnect(&self) -> Result<(), String> {
        clear_twitch_auth_state(&self.runtime).await?;
        let generation = self.runtime.next_generation();
        self.runtime.chat_status(
            TwitchStatus::Disconnected,
            Some("Twitch チャット受信を停止しました。".to_string()),
            generation,
        );
        self.runtime.auth_status(
            TwitchStatusDomain::Auth,
            TwitchStatus::Disconnected,
            Some("Twitch 連携を解除しました。".to_string()),
        );
        self.runtime
            .auth_log(AppLogLevel::Info, "Twitch 連携を解除しました。");
        Ok(())
    }
    pub(super) fn stop(&self) -> Result<(), String> {
        let stopped = self.runtime.cancel_chat()?;
        let generation = self.runtime.next_generation();
        let message = if stopped {
            "Twitch チャット受信を停止しました。"
        } else {
            "Twitch チャット受信は開始されていません。"
        };
        self.runtime.chat_status(
            TwitchStatus::Disconnected,
            Some(message.to_string()),
            generation,
        );
        self.runtime.auth_log(AppLogLevel::Info, message);
        Ok(())
    }
}
pub(super) async fn connect_validated_channel(
    channel_login: String,
    state: &impl ChatRuntime,
) -> Result<(), String> {
    let (access_token, client_id, user_id, own_login, scopes, auth_generation) = {
        let auth = state.auth().lock().map_err(|error| error.to_string())?;
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
            auth.generation,
        )
    };
    if let Err(error) = ensure_required_twitch_scopes(&scopes) {
        let message = error.to_string();
        clear_missing_scope_twitch_auth(state, &message).await?;
        return Err(message);
    }

    let channel_login = if channel_login.is_empty() {
        crate::settings::validation::TwitchLogin::parse(&own_login, false)
            .map_err(|error| error.to_string())?
            .into_string()
    } else {
        channel_login
    };
    let generation = state.next_generation();
    let channel_for_log = channel_login.clone();
    let app_for_task = (*state).clone();
    let task = tokio::spawn(async move {
        let broadcaster = match app_for_task
            .lookup_user(&client_id, &access_token, &channel_login)
            .await
        {
            Ok(broadcaster) => broadcaster,
            Err(error) => {
                let message = to_twitch_user_message(error);
                app_for_task.chat_status(TwitchStatus::Error, Some(message.clone()), generation);
                app_for_task.auth_log(AppLogLevel::Error, message);
                return;
            }
        };
        let is_current = app_for_task.connection_is_current(generation);
        if !is_current {
            return;
        }
        let params = EventSubConnectionParams {
            generation,
            auth_generation,
            broadcaster_user_id: broadcaster.id,
            broadcaster_login: broadcaster.login,
            client_id,
            user_id,
        };
        EventSubClient::new(&app_for_task).run(&params).await;
    });
    state.replace_connection(TwitchConnectionHandle::new(generation, task))?;
    state.chat_status(
        TwitchStatus::Connecting,
        Some(format!(
            "Twitch チャンネル {} に接続しています。",
            channel_for_log
        )),
        generation,
    );
    state.auth_log(
        AppLogLevel::Info,
        format!(
            "Twitch チャンネル {} への EventSub 接続を開始しました。",
            channel_for_log
        ),
    );
    Ok(())
}
