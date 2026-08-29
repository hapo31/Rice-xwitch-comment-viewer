//! Deterministic, dependency-injected Twitch state harness.
//!
//! The production commands keep their Tauri-facing API, while the stateful
//! policies are driven here through injected HTTP, WebSocket, credential-store,
//! and clock doubles. This keeps regression tests independent of Twitch, wall
//! clock timers, and the OS keyring.

use super::{
    normalize_chat_message, retry_eventsub_subscription, AuthSecretStore, AuthStorage,
    EventSubEnvelope, EventSubReconnectBackoff, MessageDedupe, SubscriptionRequestError,
    TokenResponse, TwitchApiError, TwitchAuthState, TwitchToken, TwitchUserProfile,
};
use chrono::{DateTime, Utc};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A clock that can advance without sleeping.
pub(crate) trait TestClock {
    fn now(&self) -> Instant;
    fn utc_now(&self) -> DateTime<Utc>;
}

#[derive(Clone)]
pub(crate) struct FakeClock {
    monotonic: Instant,
    wall: DateTime<Utc>,
}

impl FakeClock {
    pub(crate) fn new(wall: DateTime<Utc>) -> Self {
        Self {
            monotonic: Instant::now(),
            wall,
        }
    }

    pub(crate) fn advance(&mut self, duration: Duration) {
        self.monotonic += duration;
        self.wall += chrono::Duration::from_std(duration).expect("valid test duration");
    }
}

impl TestClock for FakeClock {
    fn now(&self) -> Instant {
        self.monotonic
    }

    fn utc_now(&self) -> DateTime<Utc> {
        self.wall
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HttpFailure {
    Unauthorized,
    Transient,
    Permanent,
}

/// The minimum HTTP surface needed to drive EventSub subscription and OAuth
/// rotation tests. It deliberately does not expose reqwest or URLs.
pub(crate) trait HttpTransport {
    fn subscribe(&mut self, access_token: &str) -> Result<(), HttpFailure>;
    fn refresh(&mut self, refresh_token: &str) -> Result<TokenResponse, HttpFailure>;
    fn validate(&mut self, access_token: &str) -> Result<TwitchUserProfile, HttpFailure>;
}

pub(crate) trait TestSocket {
    fn next_frame(&mut self) -> Option<String>;
}

pub(crate) trait WebSocketConnector {
    type Socket: TestSocket;

    fn connect(&mut self, url: &str) -> Result<Self::Socket, HttpFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StoreFailure {
    Load,
    Save,
    Clear,
}

/// A credential-store contract that can be adapted to production AuthStorage.
pub(crate) trait CredentialStore {
    fn load(&self) -> Result<Option<String>, StoreFailure>;
    fn save(&self, secret: &str) -> Result<(), StoreFailure>;
    fn clear(&self) -> Result<(), StoreFailure>;
}

#[derive(Default)]
pub(crate) struct FakeHttpTransport {
    pub subscribe_results: VecDeque<Result<(), HttpFailure>>,
    pub refresh_results: VecDeque<Result<TokenResponse, HttpFailure>>,
    pub validate_results: VecDeque<Result<TwitchUserProfile, HttpFailure>>,
    pub subscribed_tokens: Vec<String>,
    pub refreshed_tokens: Vec<String>,
    pub validated_tokens: Vec<String>,
}

impl HttpTransport for FakeHttpTransport {
    fn subscribe(&mut self, access_token: &str) -> Result<(), HttpFailure> {
        self.subscribed_tokens.push(access_token.to_string());
        self.subscribe_results.pop_front().unwrap_or(Ok(()))
    }

    fn refresh(&mut self, refresh_token: &str) -> Result<TokenResponse, HttpFailure> {
        self.refreshed_tokens.push(refresh_token.to_string());
        self.refresh_results.pop_front().unwrap_or_else(|| {
            Ok(TokenResponse {
                access_token: "refreshed-access".to_string(),
                refresh_token: "rotated-refresh".to_string(),
                scope: vec!["user:read:chat".to_string()],
                expires_in: 3600,
            })
        })
    }

    fn validate(&mut self, access_token: &str) -> Result<TwitchUserProfile, HttpFailure> {
        self.validated_tokens.push(access_token.to_string());
        self.validate_results.pop_front().unwrap_or_else(|| {
            Ok(TwitchUserProfile {
                user_id: "user-id".to_string(),
                login: "viewer".to_string(),
                client_id: "client-id".to_string(),
                scopes: vec!["user:read:chat".to_string()],
                expires_in: 3600,
            })
        })
    }
}

#[derive(Default)]
pub(crate) struct FakeWebSocketConnector {
    pub connections: VecDeque<(String, VecDeque<String>)>,
}

pub(crate) struct FakeSocket {
    frames: VecDeque<String>,
}

impl TestSocket for FakeSocket {
    fn next_frame(&mut self) -> Option<String> {
        self.frames.pop_front()
    }
}

impl WebSocketConnector for FakeWebSocketConnector {
    type Socket = FakeSocket;

    fn connect(&mut self, url: &str) -> Result<Self::Socket, HttpFailure> {
        let (expected_url, frames) = self.connections.pop_front().ok_or(HttpFailure::Transient)?;
        assert_eq!(expected_url, url);
        Ok(FakeSocket { frames })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HarnessEvent {
    Connected { session_id: String },
    Keepalive,
    Chat { id: String, text: String },
    ReconnectRequested { url: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HarnessConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    KeepaliveExpired,
}

/// Drives frame normalization, dedupe, backoff, OAuth rotation, and
/// cancellation decisions with all I/O injected.
pub(crate) struct TwitchStateHarness<C, H, W, S>
where
    C: TestClock,
    H: HttpTransport,
    W: WebSocketConnector,
    S: CredentialStore,
{
    pub clock: C,
    pub http: H,
    pub websocket: W,
    pub store: S,
    pub auth: TwitchAuthState,
    pub state: HarnessConnectionState,
    pub events: Vec<HarnessEvent>,
    pub seen_ids: MessageDedupe,
    pub backoff: EventSubReconnectBackoff,
    keepalive_deadline: Option<Instant>,
    keepalive_timeout: Duration,
    socket: Option<W::Socket>,
    connection_generation: u64,
}

impl<C, H, W, S> TwitchStateHarness<C, H, W, S>
where
    C: TestClock,
    H: HttpTransport,
    W: WebSocketConnector,
    S: CredentialStore,
{
    pub(crate) fn new(clock: C, http: H, websocket: W, store: S, auth: TwitchAuthState) -> Self {
        Self {
            clock,
            http,
            websocket,
            store,
            auth,
            state: HarnessConnectionState::Disconnected,
            events: Vec::new(),
            seen_ids: MessageDedupe::new(5_000, Duration::from_secs(600)),
            backoff: EventSubReconnectBackoff::default(),
            keepalive_deadline: None,
            keepalive_timeout: Duration::from_secs(40),
            socket: None,
            connection_generation: 0,
        }
    }

    pub(crate) fn start(&mut self, url: &str) -> Result<(), HttpFailure> {
        self.state = HarnessConnectionState::Connecting;
        self.socket = Some(self.websocket.connect(url)?);
        self.consume_until_welcome(true)
    }

    pub(crate) fn handover(&mut self, url: &str) -> Result<(), HttpFailure> {
        self.state = HarnessConnectionState::Reconnecting;
        let mut next_socket = self.websocket.connect(url)?;
        loop {
            // The old socket remains active until the new welcome arrives.
            if let Some(frame) = self.socket.as_mut().and_then(TestSocket::next_frame) {
                self.process_frame(&frame)?;
            }
            let frame = next_socket.next_frame().ok_or(HttpFailure::Transient)?;
            if self.process_frame(&frame)? {
                self.socket = Some(next_socket);
                self.finish_welcome(false)?;
                return Ok(());
            }
        }
    }

    pub(crate) fn poll(&mut self) -> Result<Option<HarnessEvent>, HttpFailure> {
        let frame = self.socket.as_mut().and_then(TestSocket::next_frame);
        frame
            .map(|frame| self.process_frame_value(&frame))
            .transpose()
    }

    pub(crate) fn keepalive_expired(&mut self) -> bool {
        let Some(deadline) = self.keepalive_deadline else {
            return false;
        };
        if self.clock.now() < deadline {
            return false;
        }
        self.state = HarnessConnectionState::KeepaliveExpired;
        self.socket = None;
        true
    }

    pub(crate) fn stop(&mut self) {
        self.connection_generation = self.connection_generation.wrapping_add(1);
        self.socket = None;
        self.state = HarnessConnectionState::Disconnected;
    }

    pub(crate) fn begin_connect(&mut self) -> u64 {
        self.connection_generation = self.connection_generation.wrapping_add(1);
        self.state = HarnessConnectionState::Connecting;
        self.connection_generation
    }

    pub(crate) fn finish_connect(&mut self, generation: u64) -> bool {
        if generation != self.connection_generation
            || self.state != HarnessConnectionState::Connecting
        {
            return false;
        }
        self.state = HarnessConnectionState::Connected;
        true
    }

    pub(crate) fn begin_validate(&self) -> (u64, String) {
        let token = self.auth.token.as_ref().expect("auth token");
        (self.auth.generation, token.access_token.clone())
    }

    pub(crate) fn finish_validate(
        &mut self,
        operation: (u64, String),
        profile: TwitchUserProfile,
    ) -> bool {
        if operation.0 != self.auth.generation {
            return false;
        }
        self.auth.profile = Some(profile);
        true
    }

    pub(crate) fn logout(&mut self) -> Result<(), StoreFailure> {
        self.store.clear()?;
        self.auth.invalidate_operations_for_test();
        self.stop();
        Ok(())
    }

    fn consume_until_welcome(&mut self, subscribe: bool) -> Result<(), HttpFailure> {
        loop {
            let frame = self
                .socket
                .as_mut()
                .and_then(TestSocket::next_frame)
                .ok_or(HttpFailure::Transient)?;
            if self.process_frame(&frame)? {
                self.finish_welcome(subscribe)?;
                return Ok(());
            }
        }
    }

    fn finish_welcome(&mut self, subscribe: bool) -> Result<(), HttpFailure> {
        if subscribe {
            let access = self
                .auth
                .token
                .as_ref()
                .expect("auth token")
                .access_token
                .clone();
            match self.http.subscribe(&access) {
                Ok(()) => {}
                Err(HttpFailure::Unauthorized) => {
                    let refresh = self
                        .auth
                        .token
                        .as_ref()
                        .expect("refresh token")
                        .refresh_token
                        .clone();
                    let token = self.http.refresh(&refresh)?;
                    let profile = self.http.validate(&token.access_token)?;
                    self.auth
                        .replace_token(token, profile)
                        .map_err(|_| HttpFailure::Permanent)?;
                    let access = self.auth.token.as_ref().unwrap().access_token.clone();
                    self.http.subscribe(&access)?;
                    self.persist_rotation()?;
                }
                Err(error) => return Err(error),
            }
        }
        self.state = HarnessConnectionState::Connected;
        self.keepalive_deadline =
            Some(self.clock.now() + self.keepalive_timeout + Duration::from_secs(5));
        self.backoff.record_session_established_at(self.clock.now());
        Ok(())
    }

    fn process_frame(&mut self, frame: &str) -> Result<bool, HttpFailure> {
        let event = self.process_frame_value(frame)?;
        Ok(matches!(event, HarnessEvent::Connected { .. }))
    }

    fn process_frame_value(&mut self, frame: &str) -> Result<HarnessEvent, HttpFailure> {
        let envelope =
            serde_json::from_str::<EventSubEnvelope>(frame).map_err(|_| HttpFailure::Permanent)?;
        let event = match envelope.metadata.message_type.as_str() {
            "session_welcome" => {
                let session = envelope.payload.session.ok_or(HttpFailure::Permanent)?;
                self.keepalive_timeout = session
                    .keepalive_timeout_seconds
                    .map(Duration::from_secs)
                    .unwrap_or(Duration::from_secs(40));
                HarnessEvent::Connected {
                    session_id: session.id,
                }
            }
            "session_keepalive" => {
                self.keepalive_deadline =
                    Some(self.clock.now() + self.keepalive_timeout + Duration::from_secs(5));
                HarnessEvent::Keepalive
            }
            "session_reconnect" => HarnessEvent::ReconnectRequested {
                url: envelope
                    .payload
                    .session
                    .and_then(|session| session.reconnect_url)
                    .ok_or(HttpFailure::Permanent)?,
            },
            "notification" => {
                let normalized = normalize_chat_message(envelope, self.clock.utc_now())
                    .map_err(|_| HttpFailure::Permanent)?;
                let Some(normalized) = normalized else {
                    return Ok(HarnessEvent::Keepalive);
                };
                let message = normalized.message;
                if !self
                    .seen_ids
                    .insert_at(message.id.clone(), self.clock.now())
                {
                    return Ok(HarnessEvent::Keepalive);
                }
                HarnessEvent::Chat {
                    id: message.id,
                    text: message.text,
                }
            }
            _ => HarnessEvent::Keepalive,
        };
        self.events.push(event.clone());
        Ok(event)
    }

    fn persist_rotation(&self) -> Result<(), HttpFailure> {
        let secret = serde_json::to_string(
            &self
                .auth
                .stored_auth_for_test()
                .ok_or(HttpFailure::Permanent)?,
        )
        .map_err(|_| HttpFailure::Permanent)?;
        self.store.save(&secret).map_err(|_| HttpFailure::Permanent)
    }
}

trait AuthStateTestExt {
    fn invalidate_operations_for_test(&mut self);
    fn stored_auth_for_test(&self) -> Option<super::StoredTwitchAuth>;
}

impl AuthStateTestExt for TwitchAuthState {
    fn invalidate_operations_for_test(&mut self) {
        self.invalidate_operations();
        self.token = None;
        self.profile = None;
    }

    fn stored_auth_for_test(&self) -> Option<super::StoredTwitchAuth> {
        self.stored_auth()
    }
}

/// Adapt a deterministic store double to production AuthStorage.
pub(crate) struct StoreAdapter<S>(pub S);

impl<S: CredentialStore> AuthSecretStore for StoreAdapter<S> {
    fn load_secret(&self) -> anyhow::Result<Option<String>> {
        self.0
            .load()
            .map_err(|error| anyhow::anyhow!(format!("{error:?}")))
    }

    fn save_secret(&self, secret: &str) -> anyhow::Result<()> {
        self.0
            .save(secret)
            .map_err(|error| anyhow::anyhow!(format!("{error:?}")))
    }

    fn clear_secret(&self) -> anyhow::Result<()> {
        self.0
            .clear()
            .map_err(|error| anyhow::anyhow!(format!("{error:?}")))
    }
}

#[derive(Default)]
pub(crate) struct FakeCredentialStore {
    pub secret: RefCell<Option<String>>,
    pub fail_load: bool,
    pub fail_save: bool,
    pub fail_clear: bool,
}

impl CredentialStore for FakeCredentialStore {
    fn load(&self) -> Result<Option<String>, StoreFailure> {
        if self.fail_load {
            Err(StoreFailure::Load)
        } else {
            Ok(self.secret.borrow().clone())
        }
    }

    fn save(&self, secret: &str) -> Result<(), StoreFailure> {
        if self.fail_save {
            Err(StoreFailure::Save)
        } else {
            *self.secret.borrow_mut() = Some(secret.to_string());
            Ok(())
        }
    }

    fn clear(&self) -> Result<(), StoreFailure> {
        if self.fail_clear {
            Err(StoreFailure::Clear)
        } else {
            *self.secret.borrow_mut() = None;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    fn at(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn auth() -> TwitchAuthState {
        TwitchAuthState {
            generation: 1,
            pending: None,
            token: Some(TwitchToken {
                access_token: "old-access".to_string(),
                refresh_token: "old-refresh".to_string(),
                scopes: vec!["user:read:chat".to_string()],
                expires_in: 3600,
            }),
            profile: Some(TwitchUserProfile {
                user_id: "user-id".to_string(),
                login: "viewer".to_string(),
                client_id: "client-id".to_string(),
                scopes: vec!["user:read:chat".to_string()],
                expires_in: 3600,
            }),
        }
    }

    fn welcome(id: &str, timeout: u64) -> String {
        serde_json::json!({
            "metadata": {"message_id": format!("welcome-{id}"), "message_type": "session_welcome"},
            "payload": {"session": {"id": id, "keepalive_timeout_seconds": timeout}}
        })
        .to_string()
    }

    fn keepalive() -> String {
        serde_json::json!({
            "metadata": {"message_id": "keepalive", "message_type": "session_keepalive"},
            "payload": {}
        })
        .to_string()
    }

    fn chat(id: &str, text: &str) -> String {
        serde_json::json!({
            "metadata": {"message_id": format!("metadata-{id}"), "message_type": "notification", "subscription_type": "channel.chat.message", "message_timestamp": "2026-08-29T00:00:00Z"},
            "payload": {"event": {
                "broadcaster_user_id": "broadcaster", "broadcaster_user_login": "streamer",
                "chatter_user_id": "chatter", "chatter_user_login": "viewer", "chatter_user_name": "Viewer",
                "message_id": id, "message": {"text": text, "fragments": []}, "badges": []
            }}
        })
        .to_string()
    }

    fn harness(
        http: FakeHttpTransport,
        websocket: FakeWebSocketConnector,
    ) -> TwitchStateHarness<FakeClock, FakeHttpTransport, FakeWebSocketConnector, FakeCredentialStore>
    {
        TwitchStateHarness::new(
            FakeClock::new(at("2026-08-29T00:00:00Z")),
            http,
            websocket,
            FakeCredentialStore::default(),
            auth(),
        )
    }

    #[test]
    fn handover_delivers_old_notification_before_new_welcome_and_dedupes_across_sessions() {
        let mut websocket = FakeWebSocketConnector::default();
        websocket.connections.push_back((
            "wss://initial".to_string(),
            VecDeque::from([welcome("old", 30), chat("same", "old")]),
        ));
        websocket.connections.push_back((
            "wss://handover".to_string(),
            VecDeque::from([
                chat("same", "duplicate"),
                welcome("new", 30),
                chat("new", "new"),
            ]),
        ));
        let mut harness = harness(FakeHttpTransport::default(), websocket);

        harness.start("wss://initial").unwrap();
        assert_eq!(
            harness.poll().unwrap(),
            Some(HarnessEvent::Chat {
                id: "same".into(),
                text: "old".into()
            })
        );
        harness.handover("wss://handover").unwrap();

        assert_eq!(harness.state, HarnessConnectionState::Connected);
        assert!(harness.events.contains(&HarnessEvent::Chat {
            id: "new".into(),
            text: "new".into()
        }));
        assert_eq!(
            harness
                .events
                .iter()
                .filter(|event| matches!(event, HarnessEvent::Chat { id, .. } if id == "same"))
                .count(),
            1
        );
    }

    #[test]
    fn keepalive_clock_expires_without_sleeping_and_keepalive_extends_deadline() {
        let mut websocket = FakeWebSocketConnector::default();
        websocket.connections.push_back((
            "wss://initial".to_string(),
            VecDeque::from([welcome("session", 10), keepalive()]),
        ));
        let mut harness = harness(FakeHttpTransport::default(), websocket);
        harness.start("wss://initial").unwrap();
        harness.clock.advance(Duration::from_secs(14));
        assert!(!harness.keepalive_expired());
        assert_eq!(harness.poll().unwrap(), Some(HarnessEvent::Keepalive));
        harness.clock.advance(Duration::from_secs(14));
        assert!(!harness.keepalive_expired());
        harness.clock.advance(Duration::from_secs(2));
        assert!(harness.keepalive_expired());
        assert_eq!(harness.state, HarnessConnectionState::KeepaliveExpired);
    }

    #[test]
    fn refresh_rotates_access_and_refresh_tokens_and_persists_the_new_secret() {
        let mut http = FakeHttpTransport::default();
        http.subscribe_results
            .push_back(Err(HttpFailure::Unauthorized));
        http.subscribe_results.push_back(Ok(()));
        http.refresh_results.push_back(Ok(TokenResponse {
            access_token: "new-access".into(),
            refresh_token: "new-refresh".into(),
            scope: vec!["user:read:chat".into()],
            expires_in: 7200,
        }));
        let mut websocket = FakeWebSocketConnector::default();
        websocket.connections.push_back((
            "wss://initial".into(),
            VecDeque::from([welcome("session", 30)]),
        ));
        let mut harness = harness(http, websocket);

        harness.start("wss://initial").unwrap();
        assert_eq!(harness.http.subscribed_tokens, ["old-access", "new-access"]);
        assert_eq!(harness.http.refreshed_tokens, ["old-refresh"]);
        assert_eq!(
            harness.auth.token.as_ref().unwrap().refresh_token,
            "new-refresh"
        );
        assert!(harness.store.secret.borrow().is_some());
    }

    #[test]
    fn validate_result_started_before_logout_is_discarded() {
        let mut harness = harness(
            FakeHttpTransport::default(),
            FakeWebSocketConnector::default(),
        );
        let operation = harness.begin_validate();
        harness.logout().unwrap();
        let accepted = harness.finish_validate(
            operation,
            TwitchUserProfile {
                user_id: "new-user".into(),
                login: "new".into(),
                client_id: "client-id".into(),
                scopes: vec!["user:read:chat".into()],
                expires_in: 3600,
            },
        );
        assert!(!accepted);
        assert!(harness.auth.token.is_none());
        assert_eq!(harness.state, HarnessConnectionState::Disconnected);
    }

    #[test]
    fn stop_invalidates_a_pending_connect_completion() {
        let mut harness = harness(
            FakeHttpTransport::default(),
            FakeWebSocketConnector::default(),
        );
        let generation = harness.begin_connect();
        harness.stop();
        assert!(!harness.finish_connect(generation));
        assert_eq!(harness.state, HarnessConnectionState::Disconnected);
    }

    #[test]
    fn transient_and_permanent_failures_have_distinct_retry_policy() {
        let transient = TwitchApiError::Http {
            status: 503,
            code: None,
            message: "down".into(),
        };
        let permanent = TwitchApiError::Http {
            status: 400,
            code: None,
            message: "bad condition".into(),
        };
        assert!(transient.is_transient());
        assert!(!permanent.is_transient());

        let mut backoff = EventSubReconnectBackoff::default();
        let start = Instant::now();
        assert_eq!(backoff.next_delay_after_failure_at(start), 2);
        assert_eq!(
            backoff.next_delay_after_failure_at(start + Duration::from_secs(1)),
            5
        );
    }

    #[test]
    fn missing_scope_is_rejected_before_subscription() {
        let mut auth = auth();
        auth.profile.as_mut().unwrap().scopes.clear();
        assert!(auth.eventsub_credentials().is_err());
    }

    #[test]
    fn partial_credential_store_failure_preserves_the_other_store() {
        let secure = FakeCredentialStore {
            fail_clear: true,
            ..Default::default()
        };
        let legacy = FakeCredentialStore::default();
        *secure.secret.borrow_mut() = Some("secure".into());
        *legacy.secret.borrow_mut() = Some("legacy".into());
        let secure_adapter = StoreAdapter(secure);
        let legacy_adapter = StoreAdapter(legacy);
        let storage = AuthStorage {
            secure: &secure_adapter,
            legacy: &legacy_adapter,
        };
        assert!(storage.clear().is_err());
        assert!(secure_adapter.0.secret.borrow().is_some());
        assert!(legacy_adapter.0.secret.borrow().is_none());
    }

    #[tokio::test]
    async fn unauthorized_subscription_refreshes_once_and_permanent_error_does_not_retry() {
        let attempts = RefCell::new(0);
        let refreshes = RefCell::new(0);
        let result = retry_eventsub_subscription(
            "expired".into(),
            |token| {
                *attempts.borrow_mut() += 1;
                async move {
                    if token == "expired" {
                        Err(SubscriptionRequestError::Unauthorized)
                    } else {
                        Ok(())
                    }
                }
            },
            || {
                *refreshes.borrow_mut() += 1;
                async { Ok("fresh".into()) }
            },
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(*attempts.borrow(), 2);
        assert_eq!(*refreshes.borrow(), 1);

        let attempts = RefCell::new(0);
        let result = retry_eventsub_subscription(
            "access".into(),
            |_| {
                *attempts.borrow_mut() += 1;
                async {
                    Err(SubscriptionRequestError::Permanent(TwitchApiError::Http {
                        status: 400,
                        code: None,
                        message: "invalid".into(),
                    }))
                }
            },
            || async { Ok("unexpected".into()) },
        )
        .await;
        assert!(matches!(
            result,
            Err(SubscriptionRequestError::Permanent(TwitchApiError::Http {
                status: 400,
                ..
            }))
        ));
        assert_eq!(*attempts.borrow(), 1);
    }
}
