//! Injected transports drive the production session, handover and supervisor.
use super::*;
use futures_util::{Sink, Stream};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio_tungstenite::tungstenite::Error;

type Frame = Result<Message, Error>;
#[derive(Debug)]
struct FakeSocket {
    frames: VecDeque<Frame>,
    pending_once: bool,
    schedule: VecDeque<tokio::time::Instant>,
    sleep: Option<Pin<Box<tokio::time::Sleep>>>,
    pong: Arc<Mutex<Vec<Message>>>,
}
impl FakeSocket {
    fn new(frames: impl IntoIterator<Item = Message>) -> Self {
        Self {
            frames: frames.into_iter().map(Ok).collect(),
            pong: Arc::default(),
            pending_once: false,
            schedule: VecDeque::new(),
            sleep: None,
        }
    }
}
impl Stream for FakeSocket {
    type Item = Frame;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Frame>> {
        use std::future::Future;
        if self.pending_once {
            self.pending_once = false;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        if let Some(at) = self.schedule.front().copied() {
            let sleep = self
                .sleep
                .get_or_insert_with(|| Box::pin(tokio::time::sleep_until(at)));
            if sleep.as_mut().poll(cx).is_pending() {
                return Poll::Pending;
            }
            self.sleep = None;
            self.schedule.pop_front();
        }
        self.frames
            .pop_front()
            .map_or(Poll::Pending, |frame| Poll::Ready(Some(frame)))
    }
}
impl Sink<Message> for FakeSocket {
    type Error = Error;
    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }
    fn start_send(self: Pin<&mut Self>, message: Message) -> Result<(), Error> {
        self.pong.lock().unwrap().push(message);
        Ok(())
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }
}
#[derive(Default)]
struct Runtime {
    sockets: Mutex<VecDeque<anyhow::Result<FakeSocket>>>,
    urls: Mutex<Vec<String>>,
    subscriptions: Mutex<Vec<(String, String)>>,
    subscription_errors: Mutex<VecDeque<anyhow::Error>>,
    chats: Mutex<Vec<ChatMessage>>,
    statuses: Mutex<Vec<TwitchStatus>>,
    logs: Mutex<Vec<String>>,
}
impl EventSubRuntime for Runtime {
    type Socket = FakeSocket;
    async fn connect(&self, url: &str) -> anyhow::Result<FakeSocket> {
        self.urls.lock().unwrap().push(url.into());
        self.sockets
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected connect")
    }
    async fn subscribe(
        &self,
        params: &EventSubConnectionParams,
        session: &str,
    ) -> anyhow::Result<()> {
        self.subscriptions
            .lock()
            .unwrap()
            .push((params.broadcaster_user_id.clone(), session.into()));
        match self.subscription_errors.lock().unwrap().pop_front() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
    fn status(&self, _: TwitchStatusDomain, status: TwitchStatus, _: Option<String>) {
        self.statuses.lock().unwrap().push(status);
    }
    fn log(&self, _: AppLogLevel, message: impl Into<String>) {
        self.logs.lock().unwrap().push(message.into());
    }
    fn chat(&self, message: ChatMessage) {
        self.chats.lock().unwrap().push(message);
    }
}
fn params() -> EventSubConnectionParams {
    EventSubConnectionParams {
        broadcaster_user_id: "broadcaster".into(),
        broadcaster_login: "streamer".into(),
        user_id: "reader".into(),
    }
}
fn welcome(id: &str) -> Message {
    Message::Text(serde_json::json!({"metadata":{"message_type":"session_welcome","message_id":"welcome"},"payload":{"session":{"id":id,"keepalive_timeout_seconds":10}}}).to_string())
}
fn chat(id: &str) -> Message {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/channel_chat_message.json")).unwrap();
    value["payload"]["event"]["message_id"] = id.into();
    Message::Text(value.to_string())
}
fn cache() -> MessageDedupe {
    MessageDedupe::new(DEDUPE_CACHE_LIMIT, DEDUPE_CACHE_TTL)
}

#[tokio::test(start_paused = true)]
async fn production_handover_preserves_ready_old_frames_and_dedupes_new_socket() {
    let runtime = Runtime::default();
    runtime
        .sockets
        .lock()
        .unwrap()
        .push_back(Ok(FakeSocket::new([
            welcome("new"),
            chat("same"),
            chat("new"),
        ])));
    let mut old = FakeSocket::new([chat("same"), chat("old-second")]);
    old.pending_once = true; // Force the new welcome branch to win once.
    let mut seen = cache();
    // Both sockets are ready in the actual select!, not an old-first simulator.
    let (mut next, session) =
        handover_eventsub_session(&runtime, &mut old, "wss://handover".into(), &mut seen)
            .await
            .unwrap();
    assert_eq!(session.id, "new");
    assert_eq!(runtime.chats.lock().unwrap().len(), 2);
    for _ in 0..2 {
        let frame = next.next().await.unwrap().unwrap();
        process_eventsub_frame(&runtime, &mut next, frame, &mut seen, Utc::now())
            .await
            .unwrap();
    }
    let ids: Vec<_> = runtime
        .chats
        .lock()
        .unwrap()
        .iter()
        .map(|chat| chat.id.clone())
        .collect();
    assert_eq!(ids, ["same", "old-second", "new"]);
    assert!(runtime.subscriptions.lock().unwrap().is_empty());
}

#[tokio::test(start_paused = true)]
async fn production_handover_failure_keeps_old_socket_until_deadline() {
    for connection in [
        Err(anyhow::anyhow!("handshake failed")),
        Ok(FakeSocket::new([Message::Close(None)])),
        Ok(FakeSocket::new([])),
    ] {
        let runtime = Runtime::default();
        runtime.sockets.lock().unwrap().push_back(connection);
        let mut old = FakeSocket::new([chat("during-failure")]);
        let start = tokio::time::Instant::now();
        let result =
            handover_eventsub_session(&runtime, &mut old, "wss://handover".into(), &mut cache())
                .await;
        assert!(result.unwrap_err().to_string().contains("通常再接続"));
        assert_eq!(start.elapsed(), EVENTSUB_RECONNECT_HANDOVER_TIMEOUT);
        assert_eq!(runtime.chats.lock().unwrap().len(), 1);
    }
}

#[tokio::test(start_paused = true)]
async fn production_session_subscribes_with_welcome_id_and_times_out() {
    let runtime = Runtime::default();
    runtime
        .sockets
        .lock()
        .unwrap()
        .push_back(Ok(FakeSocket::new([welcome("session-id"), chat("one")])));
    let start = tokio::time::Instant::now();
    let result = run_eventsub_session(
        &runtime,
        &params(),
        "wss://initial",
        &mut EventSubReconnectBackoff::default(),
        &mut cache(),
    )
    .await;
    assert!(result.unwrap_err().to_string().contains("keepalive"));
    assert_eq!(start.elapsed(), Duration::from_secs(15));
    assert_eq!(
        *runtime.subscriptions.lock().unwrap(),
        [("broadcaster".into(), "session-id".into())]
    );
}

#[tokio::test(start_paused = true)]
async fn production_supervisor_retries_transient_error_but_stops_on_terminal_error() {
    let runtime = Runtime::default();
    runtime.sockets.lock().unwrap().extend([
        Ok(FakeSocket::new([welcome("first")])),
        Ok(FakeSocket::new([welcome("second")])),
    ]);
    runtime.subscription_errors.lock().unwrap().extend([
        anyhow::anyhow!("temporary HTTP 503"),
        anyhow::Error::new(EventSubTerminalError::Permanent {
            message: "HTTP 400".into(),
        }),
    ]);
    let start = tokio::time::Instant::now();
    run_eventsub_connection_with(&runtime, &params()).await;
    assert_eq!(start.elapsed(), Duration::from_secs(2));
    assert_eq!(runtime.urls.lock().unwrap().len(), 2);
    assert!(runtime
        .statuses
        .lock()
        .unwrap()
        .iter()
        .any(|status| matches!(status, TwitchStatus::Reconnecting)));
    assert!(runtime
        .logs
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .contains("HTTP 400"));
}

#[tokio::test(start_paused = true)]
async fn production_session_reuses_dedupe_after_normal_reconnect() {
    let runtime = Runtime::default();
    runtime.sockets.lock().unwrap().extend([
        Ok(FakeSocket::new([welcome("first"), chat("same")])),
        Ok(FakeSocket::new([
            welcome("second"),
            chat("same"),
            chat("second"),
        ])),
    ]);
    let mut seen = cache();
    for _ in 0..2 {
        assert!(run_eventsub_session(
            &runtime,
            &params(),
            "wss://initial",
            &mut EventSubReconnectBackoff::default(),
            &mut seen
        )
        .await
        .is_err());
    }
    assert_eq!(runtime.chats.lock().unwrap().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn production_connection_handle_aborts_pending_session() {
    let runtime = Arc::new(Runtime::default());
    runtime
        .sockets
        .lock()
        .unwrap()
        .push_back(Ok(FakeSocket::new([])));
    let task_runtime = runtime.clone();
    let task = tokio::spawn(async move {
        run_eventsub_connection_with(task_runtime.as_ref(), &params()).await
    });
    let handle = TwitchConnectionHandle::new(1, task);
    tokio::task::yield_now().await;
    handle.abort();
    assert!(handle.task.await.unwrap_err().is_cancelled());
    assert!(runtime.subscriptions.lock().unwrap().is_empty());
}

#[derive(Default)]
struct MemoryCredentials(Mutex<Option<String>>);
impl AuthCredentialStore for MemoryCredentials {
    fn load(&self) -> AuthLoadResult {
        unreachable!("not used by rotation")
    }
    fn save(&self, auth: &TwitchAuthState) -> anyhow::Result<Option<String>> {
        *self.0.lock().unwrap() = Some(serde_json::to_string(&auth.stored_auth().unwrap())?);
        Ok(None)
    }
    fn clear(&self) -> anyhow::Result<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}
struct Http {
    calls: Mutex<Vec<String>>,
}
impl OAuthTransport for Http {
    async fn refresh(&self, client: &str, refresh: &str) -> anyhow::Result<TokenResponse> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("refresh:{client}:{refresh}"));
        Ok(TokenResponse {
            access_token: "new-access".into(),
            refresh_token: "new-refresh".into(),
            scope: vec![CHAT_READ_SCOPE.into()],
            expires_in: 3600,
        })
    }
    async fn validate(&self, access: &str) -> anyhow::Result<ValidateResponse> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("validate:{access}"));
        Ok(ValidateResponse {
            client_id: "client".into(),
            login: "viewer".into(),
            user_id: "reader".into(),
            scopes: vec![CHAT_READ_SCOPE.into()],
            expires_in: 3600,
        })
    }
}
fn auth() -> TwitchAuthState {
    TwitchAuthState {
        generation: 1,
        pending: None,
        token: Some(TwitchToken {
            access_token: "old-access".into(),
            refresh_token: "old-refresh".into(),
            scopes: vec![CHAT_READ_SCOPE.into()],
            expires_in: 3600,
        }),
        profile: Some(TwitchUserProfile {
            client_id: "client".into(),
            login: "viewer".into(),
            user_id: "reader".into(),
            scopes: vec![CHAT_READ_SCOPE.into()],
            expires_in: 3600,
        }),
    }
}
#[tokio::test]
async fn production_refresh_validates_and_saves_rotation_before_retrying_subscription() {
    let auth = Arc::new(Mutex::new(auth()));
    let backend = Arc::new(MemoryCredentials::default());
    let store = TwitchAuthStore::with_backend(backend.clone());
    let http = Http {
        calls: Mutex::default(),
    };
    let credentials = auth.lock().unwrap().eventsub_credentials().unwrap();
    let attempts = Mutex::new(Vec::new());
    retry_eventsub_subscription(
        credentials.access_token.clone(),
        |access| {
            attempts.lock().unwrap().push(access.clone());
            let saved = backend.0.lock().unwrap().clone();
            async move {
                if access == "old-access" {
                    return Err(SubscriptionRequestError::Unauthorized);
                }
                let stored: StoredTwitchAuth =
                    serde_json::from_str(&saved.expect("save before retry")).unwrap();
                assert_eq!(stored.refresh_token, "new-refresh");
                Ok(())
            }
        },
        || async {
            let (token, profile) = refresh_and_validate(&http, &credentials).await?;
            let (access, changed, warning) =
                persist_eventsub_rotation(auth.clone(), &store, &credentials, token, profile)
                    .await?;
            assert!(changed);
            assert!(warning.is_none());
            Ok(access)
        },
    )
    .await
    .unwrap();
    assert_eq!(*attempts.lock().unwrap(), ["old-access", "new-access"]);
    assert_eq!(
        *http.calls.lock().unwrap(),
        ["refresh:client:old-refresh", "validate:new-access"]
    );
}
#[tokio::test]
async fn production_validate_result_and_rotation_after_logout_cannot_restore_auth() {
    let auth = Arc::new(Mutex::new(auth()));
    let credentials = auth.lock().unwrap().eventsub_credentials().unwrap();
    let generation = auth.lock().unwrap().generation;
    let http = Http {
        calls: Mutex::default(),
    };
    let (token, profile) = refresh_and_validate(&http, &credentials).await.unwrap();
    let backend = Arc::new(MemoryCredentials::default());
    let store = TwitchAuthStore::with_backend(backend.clone());
    clear_twitch_auth_state_with_store(auth.clone(), &store)
        .await
        .unwrap();
    assert!(
        apply_validated_profile(&mut auth.lock().unwrap(), generation, profile.clone()).is_err()
    );
    assert!(
        persist_eventsub_rotation(auth.clone(), &store, &credentials, token, profile)
            .await
            .is_err()
    );
    assert!(auth.lock().unwrap().token.is_none());
    assert!(backend.0.lock().unwrap().is_none());
}

#[tokio::test(start_paused = true)]
async fn notifications_and_keepalive_reset_deadline_but_ping_does_not() {
    let runtime = Runtime::default();
    let keepalive = Message::Text(serde_json::json!({"metadata":{"message_type":"session_keepalive","message_id":"keepalive"},"payload":{}}).to_string());
    let mut socket = FakeSocket::new([
        welcome("session"),
        chat("at-eight"),
        keepalive,
        Message::Ping(vec![1]),
    ]);
    let pong = socket.pong.clone();
    let start = tokio::time::Instant::now();
    socket.schedule = [0, 8, 20, 28]
        .map(|seconds| start + Duration::from_secs(seconds))
        .into();
    runtime.sockets.lock().unwrap().push_back(Ok(socket));
    let result = run_eventsub_session(
        &runtime,
        &params(),
        "wss://initial",
        &mut EventSubReconnectBackoff::default(),
        &mut cache(),
    )
    .await;
    assert!(result.unwrap_err().to_string().contains("keepalive"));
    assert_eq!(start.elapsed(), Duration::from_secs(35));
    assert_eq!(*pong.lock().unwrap(), [Message::Pong(vec![1])]);
}

#[tokio::test(start_paused = true)]
async fn failed_handover_reaches_production_supervisor_backoff_and_resubscription() {
    let runtime = Runtime::default();
    let reconnect = Message::Text(serde_json::json!({"metadata":{"message_type":"session_reconnect","message_id":"reconnect"},"payload":{"session":{"id":"old","reconnect_url":"wss://handover"}}}).to_string());
    runtime.sockets.lock().unwrap().extend([
        Ok(FakeSocket::new([welcome("old"), reconnect, chat("during-handover")])),
        Err(anyhow::anyhow!("handshake failed")),
        Ok(FakeSocket::new([welcome("retry"), Message::Text(serde_json::json!({"metadata":{"message_type":"revocation","message_id":"revoke"},"payload":{"subscription":{"type":"channel.chat.message","status":"user_removed"}}}).to_string())])),
    ]);
    let start = tokio::time::Instant::now();
    run_eventsub_connection_with(&runtime, &params()).await;
    assert_eq!(start.elapsed(), Duration::from_secs(27));
    assert_eq!(runtime.chats.lock().unwrap().len(), 1);
    assert_eq!(runtime.subscriptions.lock().unwrap().len(), 2);
    assert_eq!(runtime.urls.lock().unwrap()[1], "wss://handover");
    assert!(runtime
        .logs
        .lock()
        .unwrap()
        .iter()
        .any(|log| log.contains("25 秒維持")));
}
