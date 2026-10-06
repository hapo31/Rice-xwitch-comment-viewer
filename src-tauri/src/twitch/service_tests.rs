//! Scripted ports exercise the same services used by the Tauri commands.
use super::auth_service::{AuthRuntime, TwitchAuthService};
use super::auth_state::*;
use super::auth_store::{AuthCredentialStore, AuthLoadResult, TwitchAuthStore};
use super::chat_service::{ChatRuntime, TwitchChatService, TwitchConnectionHandle};
use super::dedupe::MessageDedupe;
use super::error::{EventSubTerminalError, SubscriptionRequestError, TwitchApiError};
use super::eventsub::{process_eventsub_frame, run_eventsub_connection_with, EventSubRuntime};
use super::model::ChatMessage;
use super::oauth::{DeviceOAuthTransport, OAuthTransport, PollAuthError};
use super::subscription::{create_chat_message_subscription, SubscriptionRuntime};
use super::test_harness::FakeSocket;
use super::{CHAT_READ_SCOPE, DEDUPE_CACHE_LIMIT, DEDUPE_CACHE_TTL};
use crate::app_events::{
    AppEventState, AppLogEvent, AppLogLevel, TwitchAuthRequiredReason, TwitchStatus,
    TwitchStatusDomain, TwitchStatusEvent,
};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone, Default)]
struct Gate {
    started: Arc<Notify>,
    release: Arc<Notify>,
}
struct Reply<T> {
    value: T,
    gate: Option<Gate>,
}
impl<T: Send> Reply<T> {
    fn ready(value: T) -> Self {
        Self { value, gate: None }
    }
    fn delayed(value: T, gate: &Gate) -> Self {
        Self {
            value,
            gate: Some(gate.clone()),
        }
    }
    async fn receive(self) -> T {
        if let Some(gate) = self.gate {
            gate.started.notify_one();
            gate.release.notified().await;
        }
        self.value
    }
}
fn take<T>(queue: &Mutex<VecDeque<T>>) -> T {
    queue
        .lock()
        .unwrap()
        .pop_front()
        .expect("unexpected transport call")
}
#[derive(Default)]
struct MemoryStore {
    saved: Mutex<Option<TwitchAuthState>>,
    clears: AtomicUsize,
    fail_clear: AtomicBool,
    order: Arc<Mutex<Vec<&'static str>>>,
}
impl AuthCredentialStore for MemoryStore {
    fn load(&self) -> AuthLoadResult {
        AuthLoadResult {
            auth: self.saved.lock().unwrap().clone(),
            notice: None,
        }
    }
    fn save(&self, auth: &TwitchAuthState) -> anyhow::Result<Option<String>> {
        *self.saved.lock().unwrap() = Some(auth.clone());
        self.order.lock().unwrap().push("save");
        Ok(None)
    }
    fn clear(&self) -> anyhow::Result<()> {
        self.clears.fetch_add(1, Ordering::Relaxed);
        if self.fail_clear.load(Ordering::Relaxed) {
            anyhow::bail!("fake delete failure");
        }
        *self.saved.lock().unwrap() = None;
        self.order.lock().unwrap().push("clear");
        Ok(())
    }
}
#[derive(Default)]
struct Transport {
    devices: Mutex<VecDeque<Reply<anyhow::Result<DeviceCodeResponse>>>>,
    polls: Mutex<VecDeque<Reply<Result<TokenResponse, PollAuthError>>>>,
    validations: Mutex<VecDeque<anyhow::Result<ValidateResponse>>>,
    refreshes: Mutex<VecDeque<anyhow::Result<TokenResponse>>>,
    users: Mutex<VecDeque<Reply<anyhow::Result<HelixUser>>>>,
    sockets: Mutex<VecDeque<FakeSocket>>,
    subscriptions: Mutex<VecDeque<Result<(), SubscriptionRequestError>>>,
    subscription_calls: Mutex<Vec<(String, String, String)>>,
    lookup_calls: Mutex<Vec<String>>,
    validation_calls: AtomicUsize,
    refresh_calls: AtomicUsize,
}
#[derive(Clone)]
struct Runtime {
    auth: Arc<Mutex<TwitchAuthState>>,
    store: TwitchAuthStore,
    backend: Arc<MemoryStore>,
    http: Arc<Transport>,
    connection: Arc<Mutex<Option<TwitchConnectionHandle>>>,
    next_generation: Arc<AtomicU64>,
    cancels: Arc<AtomicUsize>,
    statuses: Arc<Mutex<Vec<TwitchStatus>>>,
    auth_required: Arc<AtomicUsize>,
    chats: Arc<Mutex<Vec<ChatMessage>>>,
    logs: Arc<Mutex<Vec<String>>>,
    events: Arc<AppEventState>,
    preferred: Arc<Mutex<String>>,
    clock: Arc<Mutex<SystemTime>>,
    monotonic_origin: Instant,
}
const CLOCK_EPOCH: u64 = 1_700_000_000;
impl Default for Runtime {
    fn default() -> Self {
        let backend = Arc::new(MemoryStore::default());
        Self {
            auth: Arc::default(),
            store: TwitchAuthStore::with_backend(backend.clone()),
            backend,
            http: Arc::default(),
            connection: Arc::default(),
            next_generation: Arc::new(AtomicU64::new(1)),
            cancels: Arc::default(),
            statuses: Arc::default(),
            auth_required: Arc::default(),
            chats: Arc::default(),
            logs: Arc::default(),
            events: Arc::default(),
            preferred: Arc::new(Mutex::new("preferred_streamer".into())),
            clock: Arc::new(Mutex::new(UNIX_EPOCH + Duration::from_secs(CLOCK_EPOCH))),
            monotonic_origin: Instant::now(),
        }
    }
}
impl Runtime {
    fn authorize(&self) {
        let mut auth = self.auth.lock().unwrap();
        auth.token = Some(TwitchToken {
            access_token: "old-access".into(),
            refresh_token: "old-refresh".into(),
            scopes: vec![CHAT_READ_SCOPE.into()],
            expires_in: 3600,
        });
        auth.profile = Some(profile("reader"));
    }
    fn device(&self, code: &str) {
        self.http
            .devices
            .lock()
            .unwrap()
            .push_back(Reply::ready(Ok(device(code))));
    }
    async fn wait_for_subscription(&self, count: usize) {
        for _ in 0..100 {
            if self.http.subscription_calls.lock().unwrap().len() >= count {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("subscription did not start");
    }
}
impl OAuthTransport for Runtime {
    async fn refresh(&self, _: &str, _: &str) -> anyhow::Result<TokenResponse> {
        self.http.refresh_calls.fetch_add(1, Ordering::Relaxed);
        take(&self.http.refreshes)
    }
    async fn validate(&self, _: &str) -> anyhow::Result<ValidateResponse> {
        self.http.validation_calls.fetch_add(1, Ordering::Relaxed);
        take(&self.http.validations)
    }
}
impl DeviceOAuthTransport for Runtime {
    async fn device_code(&self, _: &str) -> anyhow::Result<DeviceCodeResponse> {
        take(&self.http.devices).receive().await
    }
    async fn poll_token(&self, _: &PendingDeviceAuth) -> Result<TokenResponse, PollAuthError> {
        take(&self.http.polls).receive().await
    }
}
impl AuthRuntime for Runtime {
    fn auth(&self) -> &Arc<Mutex<TwitchAuthState>> {
        &self.auth
    }
    fn store(&self) -> &TwitchAuthStore {
        &self.store
    }
    fn client_id(&self) -> String {
        "fake-client".into()
    }
    fn now(&self) -> SystemTime {
        *self.clock.lock().unwrap()
    }
    fn cancel_chat(&self) -> Result<bool, String> {
        self.cancels.fetch_add(1, Ordering::Relaxed);
        if let Some(handle) = self.connection.lock().unwrap().take() {
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
        self.events.record_test_twitch_status(TwitchStatusEvent {
            revision: 0,
            domain,
            status: status.clone(),
            message,
            reason: None,
            connection_generation: None,
            active_connection: None,
            occurred_at_ms: 1,
        });
        if matches!(status, TwitchStatus::Connected) {
            self.backend.order.lock().unwrap().push("connected");
        }
        self.statuses.lock().unwrap().push(status);
    }
    fn auth_log(&self, level: AppLogLevel, message: impl Into<String>) {
        let message = message.into();
        self.events.record_test_log(AppLogEvent {
            id: None,
            level,
            message: message.clone(),
            occurred_at_ms: 1,
        });
        self.logs.lock().unwrap().push(message);
    }
    fn require_auth(&self, reason: TwitchAuthRequiredReason, _: impl Into<String>) {
        assert!(matches!(
            reason,
            TwitchAuthRequiredReason::MissingRequiredScope
        ));
        self.auth_required.fetch_add(1, Ordering::Relaxed);
    }
}
impl ChatRuntime for Runtime {
    fn preferred_channel(&self) -> Result<String, String> {
        Ok(self.preferred.lock().unwrap().clone())
    }
    fn next_generation(&self) -> u64 {
        self.next_generation.fetch_add(1, Ordering::Relaxed)
    }
    fn replace_connection(&self, handle: TwitchConnectionHandle) -> Result<(), String> {
        if let Some(previous) = self.connection.lock().unwrap().replace(handle) {
            previous.abort();
        }
        Ok(())
    }
    fn connection_is_current(&self, generation: u64) -> bool {
        self.connection
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|handle| handle.generation == generation)
    }
    async fn lookup_user(&self, _: &str, _: &str, login: &str) -> anyhow::Result<HelixUser> {
        self.http.lookup_calls.lock().unwrap().push(login.into());
        take(&self.http.users).receive().await
    }
}
impl SubscriptionRuntime for Runtime {
    async fn send_subscription(
        &self,
        params: &EventSubConnectionParams,
        session_id: &str,
        _: &str,
        access_token: &str,
    ) -> Result<(), SubscriptionRequestError> {
        self.backend.order.lock().unwrap().push("subscribe");
        self.http.subscription_calls.lock().unwrap().push((
            params.broadcaster_user_id.clone(),
            session_id.into(),
            access_token.into(),
        ));
        self.http
            .subscriptions
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(()))
    }
}
impl EventSubRuntime for Runtime {
    type Socket = FakeSocket;
    async fn connect(&self, _: &str) -> anyhow::Result<FakeSocket> {
        Ok(take(&self.http.sockets))
    }
    async fn subscribe(
        &self,
        params: &EventSubConnectionParams,
        session_id: &str,
    ) -> anyhow::Result<()> {
        create_chat_message_subscription(self, params, session_id).await
    }
    fn status(&self, domain: TwitchStatusDomain, status: TwitchStatus, message: Option<String>) {
        self.auth_status(domain, status, message);
    }
    fn chat_status(&self, status: TwitchStatus, message: Option<String>, generation: u64) {
        self.events.record_test_twitch_status(TwitchStatusEvent {
            revision: 0,
            domain: TwitchStatusDomain::Chat,
            status: status.clone(),
            message,
            reason: None,
            connection_generation: Some(generation),
            active_connection: None,
            occurred_at_ms: 1,
        });
        self.statuses.lock().unwrap().push(status);
    }
    fn connected(&self, _: &EventSubConnectionParams, _: String) {
        self.statuses.lock().unwrap().push(TwitchStatus::Connected);
    }
    fn log(&self, level: AppLogLevel, message: impl Into<String>) {
        self.auth_log(level, message);
    }
    fn chat(&self, mut message: ChatMessage, generation: u64) {
        message.connection_generation = Some(generation);
        self.chats.lock().unwrap().push(message);
    }
    fn received_at(&self) -> DateTime<Utc> {
        self.now().into()
    }
    fn monotonic_now(&self) -> Instant {
        self.monotonic_origin
            + self
                .now()
                .duration_since(UNIX_EPOCH + Duration::from_secs(CLOCK_EPOCH))
                .unwrap()
    }
}
fn device(code: &str) -> DeviceCodeResponse {
    DeviceCodeResponse {
        device_code: code.into(),
        user_code: "ABCD".into(),
        verification_uri: "https://example.invalid/device".into(),
        expires_in: 1800,
        interval: 5,
    }
}
fn token() -> TokenResponse {
    TokenResponse {
        access_token: "new-access".into(),
        refresh_token: "new-refresh".into(),
        scope: vec![CHAT_READ_SCOPE.into()],
        expires_in: 3600,
    }
}
fn profile(login: &str) -> TwitchUserProfile {
    validate(login).into()
}
fn validate(login: &str) -> ValidateResponse {
    ValidateResponse {
        client_id: "fake-client".into(),
        login: login.into(),
        user_id: "reader-id".into(),
        scopes: vec![CHAT_READ_SCOPE.into()],
        expires_in: 3600,
    }
}
fn api_error(status: u16, code: Option<&str>) -> anyhow::Error {
    TwitchApiError::Http {
        status,
        code: code.map(str::to_string),
        message: "認証が無効".into(),
    }
    .into()
}
fn params() -> EventSubConnectionParams {
    EventSubConnectionParams {
        generation: 1,
        broadcaster_user_id: "broadcaster-id".into(),
        broadcaster_login: "streamer".into(),
        user_id: "reader-id".into(),
    }
}
fn welcome() -> Message {
    Message::Text(serde_json::json!({"metadata":{"message_type":"session_welcome","message_id":"welcome"},"payload":{"session":{"id":"fake-session","keepalive_timeout_seconds":30}}}).to_string())
}
fn chat() -> Message {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/channel_chat_message.json")).unwrap();
    value["metadata"]["message_timestamp"] = serde_json::Value::Null;
    Message::Text(value.to_string())
}

#[tokio::test]
async fn auth_service_uses_injected_clock_and_preserves_device_payload() {
    let runtime = Runtime::default();
    runtime.device("device-one");
    let start = TwitchAuthService::new(&runtime).start().await.unwrap();
    assert_eq!(
        serde_json::to_value(start).unwrap(),
        serde_json::json!({
            "userCode": "ABCD", "verificationUri": "https://example.invalid/device",
            "expiresIn": 1800, "expiresAtMs": (CLOCK_EPOCH + 1800) * 1000, "interval": 5
        })
    );
    let auth = runtime.auth.lock().unwrap();
    let pending = auth.pending.as_ref().unwrap();
    assert_eq!(pending.device_code, "device-one");
    assert_eq!(pending.client_id, "fake-client");
    assert_eq!(pending.generation, auth.generation);
    assert!(!pending.poll_in_flight);
}

#[tokio::test]
async fn auth_service_drives_pending_slow_down_denied_expired_and_authorized() {
    let runtime = Runtime::default();
    let service = TwitchAuthService::new(&runtime);
    runtime.device("pending-device");
    service.start().await.unwrap();
    runtime.http.polls.lock().unwrap().extend([
        Reply::ready(Err(PollAuthError::Pending)),
        Reply::ready(Err(PollAuthError::SlowDown)),
        Reply::ready(Err(PollAuthError::Denied)),
    ]);
    assert!(matches!(
        service.poll().await.unwrap(),
        TwitchAuthPollResult::Pending { interval: 5, .. }
    ));
    assert!(matches!(
        service.poll().await.unwrap(),
        TwitchAuthPollResult::SlowDown { interval: 10, .. }
    ));
    assert!(matches!(
        service.poll().await.unwrap(),
        TwitchAuthPollResult::Denied { .. }
    ));
    assert!(runtime.auth.lock().unwrap().pending.is_none());
    runtime.device("expired-device");
    service.start().await.unwrap();
    runtime
        .http
        .polls
        .lock()
        .unwrap()
        .push_back(Reply::ready(Err(PollAuthError::Expired)));
    assert!(matches!(
        service.poll().await.unwrap(),
        TwitchAuthPollResult::Expired { .. }
    ));
    assert!(runtime.auth.lock().unwrap().pending.is_none());
    runtime.device("authorized-device");
    service.start().await.unwrap();
    runtime
        .http
        .polls
        .lock()
        .unwrap()
        .push_back(Reply::ready(Ok(token())));
    runtime
        .http
        .validations
        .lock()
        .unwrap()
        .push_back(Ok(validate("new-reader")));
    let result = service.poll().await.unwrap();
    assert!(matches!(
        result,
        TwitchAuthPollResult::Authorized {
            storage_warning: None,
            ..
        }
    ));
    assert_eq!(
        runtime.auth.lock().unwrap().profile.as_ref().unwrap().login,
        "new-reader"
    );
    assert_eq!(
        runtime
            .backend
            .saved
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .token
            .as_ref()
            .unwrap()
            .refresh_token,
        "new-refresh"
    );
    assert_eq!(
        *runtime.backend.order.lock().unwrap(),
        ["save", "connected"]
    );
    assert!(!runtime
        .logs
        .lock()
        .unwrap()
        .iter()
        .any(|line| line.contains("new-refresh") || line.contains("new-access")));
}

#[tokio::test]
async fn auth_service_poll_transport_error_releases_inflight_without_deleting_pending() {
    let runtime = Runtime::default();
    runtime.device("retryable-device");
    let service = TwitchAuthService::new(&runtime);
    service.start().await.unwrap();
    runtime
        .http
        .polls
        .lock()
        .unwrap()
        .push_back(Reply::ready(Err(PollAuthError::Other(api_error(
            503, None,
        )))));
    assert!(service.poll().await.is_err());
    assert!(
        !runtime
            .auth
            .lock()
            .unwrap()
            .pending
            .as_ref()
            .unwrap()
            .poll_in_flight
    );
    assert_eq!(runtime.backend.clears.load(Ordering::Relaxed), 0);
    runtime
        .http
        .polls
        .lock()
        .unwrap()
        .push_back(Reply::ready(Err(PollAuthError::Pending)));
    assert!(matches!(
        service.poll().await.unwrap(),
        TwitchAuthPollResult::Pending { .. }
    ));
}

#[tokio::test]
async fn auth_service_missing_scope_never_persists_an_authorized_token() {
    let runtime = Runtime::default();
    runtime.device("missing-scope");
    let service = TwitchAuthService::new(&runtime);
    service.start().await.unwrap();
    runtime
        .http
        .polls
        .lock()
        .unwrap()
        .push_back(Reply::ready(Ok(token())));
    let mut response = validate("reader");
    response.scopes.clear();
    runtime
        .http
        .validations
        .lock()
        .unwrap()
        .push_back(Ok(response));
    assert!(service.poll().await.is_err());
    assert!(runtime.auth.lock().unwrap().token.is_none());
    assert!(runtime.backend.saved.lock().unwrap().is_none());
    assert_eq!(runtime.auth_required.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn auth_service_rejects_old_device_response_after_new_start() {
    let runtime = Runtime::default();
    let gate = Gate::default();
    runtime
        .http
        .devices
        .lock()
        .unwrap()
        .push_back(Reply::delayed(Ok(device("old-device")), &gate));
    let old_runtime = runtime.clone();
    let old = tokio::spawn(async move { TwitchAuthService::new(&old_runtime).start().await });
    gate.started.notified().await;
    runtime.device("new-device");
    TwitchAuthService::new(&runtime).start().await.unwrap();
    gate.release.notify_one();
    assert!(old.await.unwrap().is_err());
    assert_eq!(
        runtime
            .auth
            .lock()
            .unwrap()
            .pending
            .as_ref()
            .unwrap()
            .device_code,
        "new-device"
    );
}

#[tokio::test]
async fn auth_service_serializes_polls_and_rejects_old_authorized_response() {
    let runtime = Runtime::default();
    runtime.device("old-device");
    TwitchAuthService::new(&runtime).start().await.unwrap();
    let gate = Gate::default();
    runtime
        .http
        .polls
        .lock()
        .unwrap()
        .push_back(Reply::delayed(Ok(token()), &gate));
    let old_runtime = runtime.clone();
    let old = tokio::spawn(async move { TwitchAuthService::new(&old_runtime).poll().await });
    gate.started.notified().await;
    assert!(TwitchAuthService::new(&runtime).poll().await.is_err());
    runtime.device("new-device");
    TwitchAuthService::new(&runtime).start().await.unwrap();
    gate.release.notify_one();
    assert!(old.await.unwrap().is_err());
    assert_eq!(runtime.http.validation_calls.load(Ordering::Relaxed), 0);
    assert!(runtime.backend.saved.lock().unwrap().is_none());
    assert!(
        !runtime
            .auth
            .lock()
            .unwrap()
            .pending
            .as_ref()
            .unwrap()
            .poll_in_flight
    );
}

#[tokio::test]
async fn auth_service_validate_refresh_saves_rotation_before_connected_event() {
    let runtime = Runtime::default();
    runtime.authorize();
    runtime
        .http
        .validations
        .lock()
        .unwrap()
        .extend([Err(api_error(401, None)), Ok(validate("rotated-reader"))]);
    runtime
        .http
        .refreshes
        .lock()
        .unwrap()
        .push_back(Ok(token()));
    let result = TwitchAuthService::new(&runtime).validate().await.unwrap();
    assert_eq!(result.profile.login, "rotated-reader");
    assert_eq!(
        runtime
            .backend
            .saved
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .token
            .as_ref()
            .unwrap()
            .refresh_token,
        "new-refresh"
    );
    assert_eq!(
        *runtime.backend.order.lock().unwrap(),
        ["save", "connected"]
    );
    assert_eq!(runtime.http.refresh_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn auth_service_validate_transient_failure_keeps_auth_but_invalid_grant_clears_it() {
    for definitive in [false, true] {
        let runtime = Runtime::default();
        runtime.authorize();
        runtime
            .http
            .validations
            .lock()
            .unwrap()
            .push_back(Err(api_error(401, None)));
        runtime
            .http
            .refreshes
            .lock()
            .unwrap()
            .push_back(Err(if definitive {
                api_error(400, Some("invalid_grant"))
            } else {
                api_error(503, None)
            }));
        assert!(TwitchAuthService::new(&runtime).validate().await.is_err());
        assert_eq!(runtime.auth.lock().unwrap().token.is_none(), definitive);
        assert_eq!(
            runtime.backend.clears.load(Ordering::Relaxed),
            usize::from(definitive)
        );
        assert_eq!(
            runtime.cancels.load(Ordering::Relaxed),
            usize::from(definitive)
        );
    }
}

#[tokio::test]
async fn auth_service_validate_without_refresh_preserves_wire_profile() {
    let runtime = Runtime::default();
    runtime.authorize();
    runtime
        .http
        .validations
        .lock()
        .unwrap()
        .push_back(Ok(validate("valid-reader")));
    let result = TwitchAuthService::new(&runtime).validate().await.unwrap();
    let payload = serde_json::to_value(result).unwrap();
    assert_eq!(payload["profile"]["login"], "valid-reader");
    assert!(payload["profile"].get("clientId").is_none());
    assert!(payload.get("storageWarning").is_none());
    assert_eq!(runtime.http.refresh_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        *runtime.backend.order.lock().unwrap(),
        ["save", "connected"]
    );
}

#[tokio::test]
async fn chat_service_failed_logout_restores_credentials_without_aborting_connection() {
    let runtime = Runtime::default();
    runtime.authorize();
    runtime.backend.fail_clear.store(true, Ordering::Relaxed);
    let task = tokio::spawn(std::future::pending::<()>());
    let abort = task.abort_handle();
    runtime
        .replace_connection(TwitchConnectionHandle::new(5, task))
        .unwrap();
    let generation = runtime.auth.lock().unwrap().generation;
    assert!(TwitchChatService::new(runtime.clone())
        .disconnect()
        .await
        .is_err());
    let auth = runtime.auth.lock().unwrap();
    assert!(auth.token.is_some());
    assert_ne!(auth.generation, generation);
    drop(auth);
    assert!(runtime.connection_is_current(5));
    assert!(!abort.is_finished());
    assert_eq!(runtime.cancels.load(Ordering::Relaxed), 0);
    runtime.cancel_chat().unwrap();
}

#[tokio::test]
async fn chat_service_rejects_invalid_channel_before_any_transport_or_generation_change() {
    let runtime = Runtime::default();
    runtime.authorize();
    let service = TwitchChatService::new(runtime.clone());
    assert!(service
        .connect(Some("https://twitch.tv/invalid".into()))
        .await
        .is_err());
    *runtime.preferred.lock().unwrap() = "bad channel!".into();
    assert!(service.connect(None).await.is_err());
    assert_eq!(runtime.next_generation.load(Ordering::Relaxed), 1);
    assert!(runtime.http.lookup_calls.lock().unwrap().is_empty());
    assert!(runtime.connection.lock().unwrap().is_none());
    assert!(runtime.auth.lock().unwrap().token.is_some());
}

#[tokio::test]
async fn chat_service_connects_preferred_channel_and_stop_preserves_authentication() {
    let runtime = Runtime::default();
    runtime.authorize();
    runtime
        .http
        .users
        .lock()
        .unwrap()
        .push_back(Reply::ready(Ok(HelixUser {
            id: "broadcaster-id".into(),
            login: "preferred_streamer".into(),
        })));
    runtime
        .http
        .sockets
        .lock()
        .unwrap()
        .push_back(FakeSocket::new([welcome(), chat()]));
    let service = TwitchChatService::new(runtime.clone());
    service.connect(None).await.unwrap();
    runtime.wait_for_subscription(1).await;
    for _ in 0..10 {
        if !runtime.chats.lock().unwrap().is_empty() {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(
        *runtime.http.lookup_calls.lock().unwrap(),
        ["preferred_streamer"]
    );
    assert_eq!(
        runtime.http.subscription_calls.lock().unwrap()[0],
        (
            "broadcaster-id".into(),
            "fake-session".into(),
            "old-access".into()
        )
    );
    assert_eq!(
        runtime.chats.lock().unwrap()[0].connection_generation,
        Some(1)
    );
    let abort = runtime
        .connection
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .task
        .abort_handle();
    service.stop().unwrap();
    tokio::task::yield_now().await;
    assert!(abort.is_finished());
    assert!(runtime.auth.lock().unwrap().token.is_some());
    assert_eq!(runtime.backend.clears.load(Ordering::Relaxed), 0);
    assert_eq!(runtime.next_generation.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn chat_service_replacement_aborts_pending_old_lookup_and_logout_clears_current_auth() {
    let runtime = Runtime::default();
    runtime.authorize();
    let gate = Gate::default();
    runtime.http.users.lock().unwrap().extend([
        Reply::delayed(
            Ok(HelixUser {
                id: "old-broadcaster".into(),
                login: "old".into(),
            }),
            &gate,
        ),
        Reply::ready(Ok(HelixUser {
            id: "new-broadcaster".into(),
            login: "new".into(),
        })),
    ]);
    runtime
        .http
        .sockets
        .lock()
        .unwrap()
        .push_back(FakeSocket::new([welcome()]));
    let service = TwitchChatService::new(runtime.clone());
    service.connect(Some("old".into())).await.unwrap();
    gate.started.notified().await;
    let old = runtime
        .connection
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .task
        .abort_handle();
    service.connect(Some("new".into())).await.unwrap();
    runtime.wait_for_subscription(1).await;
    assert!(old.is_finished());
    assert!(runtime.connection_is_current(2));
    assert_eq!(
        runtime.http.subscription_calls.lock().unwrap()[0].0,
        "new-broadcaster"
    );
    service.disconnect().await.unwrap();
    assert!(runtime.auth.lock().unwrap().token.is_none());
    assert!(runtime.connection.lock().unwrap().is_none());
    assert_eq!(runtime.backend.clears.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn subscription_service_refreshes_once_and_persists_before_retry() {
    let runtime = Runtime::default();
    runtime.authorize();
    runtime
        .http
        .subscriptions
        .lock()
        .unwrap()
        .extend([Err(SubscriptionRequestError::Unauthorized), Ok(())]);
    runtime
        .http
        .refreshes
        .lock()
        .unwrap()
        .push_back(Ok(token()));
    runtime
        .http
        .validations
        .lock()
        .unwrap()
        .push_back(Ok(validate("reader")));
    create_chat_message_subscription(&runtime, &params(), "session")
        .await
        .unwrap();
    let calls = runtime.http.subscription_calls.lock().unwrap();
    assert_eq!(
        calls.iter().map(|call| call.2.as_str()).collect::<Vec<_>>(),
        ["old-access", "new-access"]
    );
    assert_eq!(runtime.http.refresh_calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        runtime
            .backend
            .saved
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .token
            .as_ref()
            .unwrap()
            .refresh_token,
        "new-refresh"
    );
    assert_eq!(
        *runtime.backend.order.lock().unwrap(),
        ["subscribe", "save", "subscribe"]
    );
}

#[tokio::test]
async fn subscription_service_uses_typed_failures_not_japanese_display_text() {
    for status in [400, 403, 503] {
        let runtime = Runtime::default();
        runtime.authorize();
        let error = TwitchApiError::Http {
            status,
            code: None,
            message: "認証が無効".into(),
        };
        runtime
            .http
            .subscriptions
            .lock()
            .unwrap()
            .push_back(Err(if status >= 500 {
                SubscriptionRequestError::Retryable(error.into())
            } else {
                SubscriptionRequestError::Permanent(error)
            }));
        let result = create_chat_message_subscription(&runtime, &params(), "session")
            .await
            .unwrap_err();
        assert_eq!(
            result.downcast_ref::<EventSubTerminalError>().is_some(),
            status < 500
        );
        assert_eq!(runtime.auth.lock().unwrap().token.is_none(), status == 403);
        assert_eq!(
            runtime.backend.clears.load(Ordering::Relaxed),
            usize::from(status == 403)
        );
        assert_eq!(runtime.http.refresh_calls.load(Ordering::Relaxed), 0);
        assert_eq!(runtime.http.subscription_calls.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn eventsub_frame_uses_injected_monotonic_and_receive_clocks() {
    let runtime = Runtime::default();
    let mut socket = FakeSocket::new([]);
    let mut seen = MessageDedupe::new(DEDUPE_CACHE_LIMIT, DEDUPE_CACHE_TTL);
    for _ in 0..2 {
        process_eventsub_frame(
            &runtime,
            &mut socket,
            chat(),
            &mut seen,
            runtime.received_at(),
            9,
        )
        .await
        .unwrap();
    }
    assert_eq!(runtime.chats.lock().unwrap().len(), 1);
    *runtime.clock.lock().unwrap() += DEDUPE_CACHE_TTL;
    process_eventsub_frame(
        &runtime,
        &mut socket,
        chat(),
        &mut seen,
        runtime.received_at(),
        9,
    )
    .await
    .unwrap();
    let chats = runtime.chats.lock().unwrap();
    assert_eq!(chats.len(), 2);
    assert_ne!(chats[0].received_at, chats[1].received_at);
    assert_eq!(chats[1].connection_generation, Some(9));
}

#[tokio::test]
async fn terminal_subscription_records_final_snapshot_before_task_exit() {
    for http_status in [400, 410, 401, 403] {
        let runtime = Runtime::default();
        runtime.authorize();
        runtime
            .http
            .sockets
            .lock()
            .unwrap()
            .push_back(FakeSocket::new([welcome()]));
        if http_status == 401 {
            runtime.http.subscriptions.lock().unwrap().extend([
                Err(SubscriptionRequestError::Unauthorized),
                Err(SubscriptionRequestError::Unauthorized),
            ]);
            runtime
                .http
                .refreshes
                .lock()
                .unwrap()
                .push_back(Ok(token()));
            runtime
                .http
                .validations
                .lock()
                .unwrap()
                .push_back(Ok(validate("reader")));
        } else {
            runtime.http.subscriptions.lock().unwrap().push_back(Err(
                SubscriptionRequestError::Permanent(TwitchApiError::Http {
                    status: http_status,
                    code: None,
                    message: "fake HTTP failure".into(),
                }),
            ));
        }
        let task_runtime = runtime.clone();
        tokio::spawn(async move { run_eventsub_connection_with(&task_runtime, &params()).await })
            .await
            .unwrap();

        let snapshot = runtime.events.snapshot();
        let chat = snapshot
            .twitch_statuses
            .iter()
            .find(|status| status.domain == TwitchStatusDomain::Chat)
            .unwrap();
        assert_eq!(chat.connection_generation, Some(params().generation));
        let requires_auth = matches!(http_status, 401 | 403);
        if requires_auth {
            assert!(matches!(chat.status, TwitchStatus::AuthRequired));
            assert!(snapshot.twitch_statuses.iter().any(|status| {
                status.domain == TwitchStatusDomain::Auth
                    && matches!(status.status, TwitchStatus::AuthRequired)
                    && status.message.is_some()
            }));
        } else {
            assert!(matches!(chat.status, TwitchStatus::Error));
            assert!(chat
                .message
                .as_ref()
                .is_some_and(|message| !message.is_empty()));
        }
        assert!(snapshot
            .logs
            .iter()
            .any(|log| matches!(log.level, AppLogLevel::Error)));
        assert!(!runtime
            .statuses
            .lock()
            .unwrap()
            .iter()
            .any(|status| matches!(status, TwitchStatus::Reconnecting)));
        assert_eq!(
            runtime.http.subscription_calls.lock().unwrap().len(),
            if http_status == 401 { 2 } else { 1 }
        );
        assert_eq!(
            runtime.http.refresh_calls.load(Ordering::Relaxed),
            usize::from(http_status == 401)
        );
    }
}

#[tokio::test]
async fn revoked_subscription_has_one_terminal_chat_transition_and_recovery_snapshot() {
    for reason in [
        "authorization_revoked",
        "user_removed",
        "version_removed",
        "unknown",
    ] {
        let runtime = Runtime::default();
        runtime.authorize();
        let revocation = Message::Text(
            serde_json::json!({
                "metadata": { "message_type": "revocation", "message_id": "revoked" },
                "payload": { "subscription": { "type": "channel.chat.message", "status": reason } }
            })
            .to_string(),
        );
        runtime
            .http
            .sockets
            .lock()
            .unwrap()
            .push_back(FakeSocket::new([welcome(), revocation]));
        run_eventsub_connection_with(&runtime, &params()).await;
        let snapshot = runtime.events.snapshot();
        let chat = snapshot
            .twitch_statuses
            .iter()
            .find(|status| status.domain == TwitchStatusDomain::Chat)
            .unwrap();
        assert_eq!(chat.connection_generation, Some(params().generation));
        if reason == "authorization_revoked" {
            assert!(matches!(chat.status, TwitchStatus::AuthRequired));
            assert!(snapshot
                .twitch_statuses
                .iter()
                .any(|status| status.domain == TwitchStatusDomain::Auth
                    && matches!(status.status, TwitchStatus::AuthRequired)));
        } else {
            assert!(matches!(chat.status, TwitchStatus::Error));
            assert!(chat.message.as_ref().unwrap().contains(reason));
            assert_eq!(
                runtime
                    .statuses
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|status| matches!(status, TwitchStatus::Error))
                    .count(),
                1
            );
        }
        assert!(snapshot
            .logs
            .iter()
            .any(|log| matches!(log.level, AppLogLevel::Error) && log.message.contains(reason)));
        assert_eq!(runtime.http.subscription_calls.lock().unwrap().len(), 1);
    }
}

#[test]
fn service_boundaries_and_command_names_remain_explicit() {
    for source in [
        include_str!("auth_service.rs"),
        include_str!("chat_service.rs"),
        include_str!("eventsub.rs"),
    ] {
        for forbidden in [
            "tauri::",
            "reqwest::",
            "keyring::",
            "use super::*",
            "emit_app_log(",
        ] {
            assert!(
                !source.contains(forbidden),
                "service depends on infrastructure: {forbidden}"
            );
        }
    }
    let commands = include_str!("commands.rs");
    assert_eq!(commands.matches("#[tauri::command]").count(), 7);
    for name in [
        "twitch_start_auth",
        "twitch_poll_auth",
        "twitch_validate_auth",
        "twitch_get_stored_auth",
        "twitch_connect",
        "twitch_disconnect",
        "twitch_stop_chat",
    ] {
        assert!(commands.contains(&format!("fn {name}(")));
    }
    for forbidden in [
        "reqwest::",
        "keyring::",
        "tokio::spawn",
        "twitch_http_client",
        ".twitch_connection",
    ] {
        assert!(
            !commands.contains(forbidden),
            "command contains infrastructure: {forbidden}"
        );
    }
}
