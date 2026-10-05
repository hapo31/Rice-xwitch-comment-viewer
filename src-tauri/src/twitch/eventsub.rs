//! Twitch eventsub responsibility boundary.
#[cfg(feature = "app")]
use super::auth_state::EventSubConnectionParams;
#[cfg(feature = "app")]
use super::dedupe::MessageDedupe;
#[cfg(feature = "app")]
use super::error::EventSubTerminalError;
#[cfg(feature = "app")]
use super::model::ChatMessage;
#[cfg(feature = "app")]
use super::normalization::{normalize_chat_message, EventSubEnvelope, EventSubSession};
#[cfg(feature = "app")]
use super::{
    DEDUPE_CACHE_LIMIT, DEDUPE_CACHE_TTL, EVENTSUB_BACKOFF_RESET_STABLE_DURATION,
    EVENTSUB_RECONNECT_HANDOVER_TIMEOUT, TWITCH_EVENTSUB_WS_URL, TWITCH_WS_HANDSHAKE_TIMEOUT,
};
#[cfg(feature = "app")]
use crate::app_events::{AppLogLevel, TwitchStatus, TwitchStatusDomain};
#[cfg(feature = "app")]
use chrono::{DateTime, Utc};
#[cfg(feature = "app")]
use futures_util::{FutureExt, SinkExt, StreamExt};
#[cfg(feature = "app")]
use std::time::{Duration, Instant};
#[cfg(feature = "app")]
use tokio::net::TcpStream;
#[cfg(feature = "app")]
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

#[cfg(feature = "app")]
pub(super) type EventSubSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[cfg(feature = "app")]
pub(super) enum EventSubFrameAction {
    Continue,
    Activity,
    Welcome(EventSubSession),
    Reconnect(String),
}

#[cfg(feature = "app")]
#[derive(Debug, Default)]
pub(super) struct EventSubReconnectBackoff {
    pub(super) failed_attempts: u64,
    pub(super) established_at: Option<Instant>,
}

#[cfg(feature = "app")]
impl EventSubReconnectBackoff {
    pub(super) fn record_session_established_at(&mut self, established_at: Instant) {
        self.established_at = Some(established_at);
    }

    pub(super) fn record_handover_started(&mut self) {
        self.established_at = None;
    }

    pub(super) fn next_delay_after_failure_at(&mut self, failed_at: Instant) -> u64 {
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

#[cfg(feature = "app")]
pub(super) trait EventSubRuntime: Sync {
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
    fn received_at(&self) -> DateTime<Utc> {
        Utc::now()
    }
    fn monotonic_now(&self) -> Instant {
        tokio::time::Instant::now().into_std()
    }
}

#[cfg(feature = "app")]
pub(super) struct EventSubClient<'a, R> {
    runtime: &'a R,
}
#[cfg(feature = "app")]
impl<'a, R: EventSubRuntime> EventSubClient<'a, R> {
    pub(super) fn new(runtime: &'a R) -> Self {
        Self { runtime }
    }
    pub(super) async fn run(&self, params: &EventSubConnectionParams) {
        run_eventsub_connection_with(self.runtime, params).await;
    }
}

#[cfg(feature = "app")]
pub(super) async fn run_eventsub_connection_with<R: EventSubRuntime>(
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
                if matches!(terminal, EventSubTerminalError::ObsoleteConnection) {
                    break;
                }
                // The supervisor owns the terminal Chat transition for both API
                // errors and revocations. Record it before this task exits.
                let (status, message) = match terminal {
                    EventSubTerminalError::AuthRequired { .. } => {
                        // Auth already carries the recovery instruction. Keep the
                        // Chat state current without duplicating its system entry.
                        (TwitchStatus::AuthRequired, None)
                    }
                    EventSubTerminalError::Permanent { message } => {
                        (TwitchStatus::Error, Some(message.clone()))
                    }
                    EventSubTerminalError::ObsoleteConnection => {
                        unreachable!("obsolete EventSub connections exit before publishing status")
                    }
                };
                app.chat_status(status, message, params.generation);
                app.log(AppLogLevel::Error, terminal.to_string());
                break;
            }
            let wait_seconds = reconnect_backoff.next_delay_after_failure_at(app.monotonic_now());
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
pub(super) async fn run_eventsub_session<R: EventSubRuntime>(
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
            app.received_at(),
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
pub(super) async fn complete_eventsub_welcome<R: EventSubRuntime>(
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

    reconnect_backoff.record_session_established_at(app.monotonic_now());
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
pub(super) async fn handover_eventsub_session<R: EventSubRuntime>(
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
                    match process_eventsub_frame(app, old_socket, old_message, seen_message_ids, app.received_at(), connection_generation).await? {
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
                        Ok(Ok(new_message)) => match process_eventsub_frame(app, new_socket, new_message, seen_message_ids, app.received_at(), connection_generation).await {
                            Ok(EventSubFrameAction::Welcome(session)) => {
                                // The old stream may become ready while the new welcome is polled.
                                // Drain already available frames before replacing it; never wait for
                                // a future old frame or exceed the handover deadline.
                                while tokio::time::Instant::now() < deadline {
                                    match old_socket.next().now_or_never() {
                                        Some(Some(Ok(Message::Close(_)))) | Some(Some(Err(_))) | Some(None) | None => break,
                                        Some(Some(Ok(frame))) => {
                                            process_eventsub_frame(app, old_socket, frame, seen_message_ids, app.received_at(), connection_generation).await?;
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
                            Err(error) if error.is::<EventSubTerminalError>() => return Err(error),
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
                    match process_eventsub_frame(app, old_socket, old_message, seen_message_ids, app.received_at(), connection_generation).await? {
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
                    match process_eventsub_frame(app, old_socket, old_message, seen_message_ids, app.received_at(), connection_generation).await? {
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
pub(super) async fn process_eventsub_frame<R: EventSubRuntime>(
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
                        if seen_message_ids.insert_at(dedupe_id, app.monotonic_now()) {
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
                            app.status(
                                TwitchStatusDomain::Auth,
                                TwitchStatus::AuthRequired,
                                Some(message.clone()),
                            );
                            EventSubTerminalError::AuthRequired { message }
                        }
                        Some("user_removed") => {
                            let message = format!("Twitch EventSub の対象ユーザーが存在しません。接続チャンネルを確認してください: {reason}");
                            EventSubTerminalError::Permanent { message }
                        }
                        Some("version_removed") => {
                            let message = format!("Twitch EventSub の購読バージョンが廃止されました。アプリを更新してください: {reason}");
                            EventSubTerminalError::Permanent { message }
                        }
                        _ => {
                            let message = format!("Twitch EventSub 購読が取り消されました。再接続せず停止します: {reason}");
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

pub(super) fn retry_backoff_seconds(attempt: u64) -> u64 {
    match attempt {
        0 | 1 => 2,
        2 => 5,
        3 => 10,
        _ => 30,
    }
}
