#[cfg(feature = "app")]
use super::{
    clear_auth_for_eventsub_missing_scope_if_current, clear_twitch_auth_state_with_store,
    restore_auth_after_failed_clear_if_current, restore_stored_auth, retry_eventsub_subscription,
    AuthClearOutcome, AuthCredentialStore, AuthLoadResult, AuthSaveOutcome, AuthSecretStore,
    AuthStorage, EventSubReconnectBackoff, StoredTwitchAuth, SubscriptionRequestError,
    TokenResponse, TwitchAuthState, TwitchAuthStore, TwitchConnectionHandle, TwitchToken,
    EVENTSUB_BACKOFF_RESET_STABLE_DURATION, EVENTSUB_RECONNECT_HANDOVER_TIMEOUT,
    TWITCH_WS_HANDSHAKE_TIMEOUT,
};
use super::{
    ensure_required_twitch_scopes, is_definitive_auth_failure, normalize_chat_message,
    oauth_error_code, retry_backoff_seconds, ChatEmote, EventSubEnvelope, MessageDedupe,
    MessageFragment, OAuthErrorResponse, TwitchApiError, TwitchAuthFailure, TwitchAuthPollResult,
    TwitchAuthValidationResult, TwitchUserProfile, CHAT_READ_SCOPE, TWITCH_HTTP_TIMEOUT,
};
use chrono::{DateTime, Utc};
#[cfg(feature = "app")]
use std::cell::RefCell;
use std::time::{Duration, Instant};

#[cfg(feature = "app")]
use std::sync::{Arc, Condvar, Mutex};

#[cfg(feature = "app")]
#[derive(Default)]
struct FakeAuthSecretStore {
    secret: RefCell<Option<String>>,
    fail_load: bool,
    fail_save: bool,
    fail_clear: bool,
    save_calls: RefCell<usize>,
}

#[cfg(feature = "app")]
impl FakeAuthSecretStore {
    fn with_secret(secret: String) -> Self {
        Self {
            secret: RefCell::new(Some(secret)),
            ..Self::default()
        }
    }
}

#[cfg(feature = "app")]
impl AuthSecretStore for FakeAuthSecretStore {
    fn load_secret(&self) -> anyhow::Result<Option<String>> {
        if self.fail_load {
            return Err(anyhow::anyhow!("fake secure-store read failure"));
        }
        Ok(self.secret.borrow().clone())
    }

    fn save_secret(&self, secret: &str) -> anyhow::Result<()> {
        *self.save_calls.borrow_mut() += 1;
        if self.fail_save {
            return Err(anyhow::anyhow!("fake secure-store write failure"));
        }
        *self.secret.borrow_mut() = Some(secret.to_string());
        Ok(())
    }

    fn clear_secret(&self) -> anyhow::Result<()> {
        if self.fail_clear {
            return Err(anyhow::anyhow!("fake secure-store clear failure"));
        }
        *self.secret.borrow_mut() = None;
        Ok(())
    }
}

#[cfg(feature = "app")]
#[derive(Default)]
struct DelayedCredentialStore {
    state: Mutex<DelayedCredentialState>,
    changed: Condvar,
}

#[cfg(feature = "app")]
struct DelayedCredentialState {
    secret: Option<String>,
    save_started: bool,
    release_save: bool,
    clear_started: bool,
    release_clear: bool,
    fail_clear: bool,
    save_calls: usize,
    clear_calls: usize,
}

#[cfg(feature = "app")]
impl Default for DelayedCredentialState {
    fn default() -> Self {
        Self {
            secret: None,
            save_started: false,
            release_save: false,
            clear_started: false,
            // Existing save/clear tests do not need a delayed clear. Keep it
            // open unless the production-path regression test opts in.
            release_clear: true,
            fail_clear: false,
            save_calls: 0,
            clear_calls: 0,
        }
    }
}

#[cfg(feature = "app")]
impl DelayedCredentialStore {
    fn wait_until_save_started(&self) {
        let mut state = self.state.lock().unwrap();
        while !state.save_started {
            state = self.changed.wait(state).unwrap();
        }
    }

    fn release_save(&self) {
        self.state.lock().unwrap().release_save = true;
        self.changed.notify_all();
    }

    fn delay_and_fail_clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.release_clear = false;
        state.fail_clear = true;
    }

    fn delay_clear(&self) {
        self.state.lock().unwrap().release_clear = false;
    }

    fn wait_until_clear_started(&self) {
        let mut state = self.state.lock().unwrap();
        while !state.clear_started {
            state = self.changed.wait(state).unwrap();
        }
    }

    fn release_clear(&self) {
        self.state.lock().unwrap().release_clear = true;
        self.changed.notify_all();
    }

    fn snapshot(&self) -> DelayedCredentialState {
        let state = self.state.lock().unwrap();
        DelayedCredentialState {
            secret: state.secret.clone(),
            save_started: state.save_started,
            release_save: state.release_save,
            clear_started: state.clear_started,
            release_clear: state.release_clear,
            fail_clear: state.fail_clear,
            save_calls: state.save_calls,
            clear_calls: state.clear_calls,
        }
    }
}

#[cfg(feature = "app")]
impl AuthCredentialStore for DelayedCredentialStore {
    fn load(&self) -> AuthLoadResult {
        AuthLoadResult {
            auth: None,
            storage_warning: None,
        }
    }

    fn save(&self, auth: &TwitchAuthState) -> anyhow::Result<Option<String>> {
        let mut state = self.state.lock().unwrap();
        state.save_started = true;
        self.changed.notify_all();
        while !state.release_save {
            state = self.changed.wait(state).unwrap();
        }
        state.save_calls += 1;
        state.secret = Some(serde_json::to_string(&auth.stored_auth().unwrap())?);
        Ok(None)
    }

    fn clear(&self) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.clear_started = true;
        self.changed.notify_all();
        while !state.release_clear {
            state = self.changed.wait(state).unwrap();
        }
        state.clear_calls += 1;
        if state.fail_clear {
            return Err(anyhow::anyhow!("fake delayed credential clear failure"));
        }
        state.secret = None;
        Ok(())
    }
}

#[cfg(feature = "app")]
fn stored_auth_secret() -> String {
    serde_json::to_string(&StoredTwitchAuth {
        client_id: "client-id".to_string(),
        access_token: "access-token".to_string(),
        refresh_token: "refresh-token".to_string(),
        scopes: vec!["user:read:chat".to_string()],
        expires_in: 3600,
        profile: TwitchUserProfile {
            user_id: "user-id".to_string(),
            login: "viewer".to_string(),
            client_id: "client-id".to_string(),
            scopes: vec!["user:read:chat".to_string()],
            expires_in: 3600,
        },
    })
    .unwrap()
}

#[cfg(feature = "app")]
fn twitch_auth_state() -> TwitchAuthState {
    TwitchAuthState {
        generation: 0,
        pending: None,
        token: Some(TwitchToken {
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
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

#[cfg(feature = "app")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delayed_credential_save_does_not_hold_auth_mutex() {
    let backend = Arc::new(DelayedCredentialStore::default());
    let store = TwitchAuthStore::with_backend(backend.clone());
    let auth_state = Arc::new(Mutex::new(twitch_auth_state()));
    let generation = auth_state.lock().unwrap().generation;

    let save = tokio::spawn({
        let store = store.clone();
        let auth_state = auth_state.clone();
        async move {
            store
                .save_if_current(auth_state, generation, twitch_auth_state())
                .await
        }
    });
    backend.wait_until_save_started();

    // A delayed keyring write must not monopolize the state mutex.
    assert_eq!(
        auth_state
            .lock()
            .unwrap()
            .profile
            .as_ref()
            .map(|profile| profile.login.as_str()),
        Some("viewer")
    );

    backend.release_save();
    assert!(matches!(
        save.await.unwrap().unwrap(),
        AuthSaveOutcome::Saved(None)
    ));
}

#[cfg(feature = "app")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn logout_clears_a_delayed_save_that_started_before_logout() {
    let backend = Arc::new(DelayedCredentialStore::default());
    let store = TwitchAuthStore::with_backend(backend.clone());
    let auth_state = Arc::new(Mutex::new(twitch_auth_state()));
    let generation = auth_state.lock().unwrap().generation;

    let save = tokio::spawn({
        let store = store.clone();
        let auth_state = auth_state.clone();
        async move {
            store
                .save_if_current(auth_state, generation, twitch_auth_state())
                .await
        }
    });
    backend.wait_until_save_started();

    // Logout invalidates the generation before waiting for credential I/O.
    let logout_generation = {
        let mut auth = auth_state.lock().unwrap();
        let generation = auth.invalidate_operations();
        auth.token = None;
        auth.profile = None;
        generation
    };
    let clear = tokio::spawn({
        let store = store.clone();
        let auth_state = auth_state.clone();
        async move { store.clear_if_current(auth_state, logout_generation).await }
    });

    backend.release_save();
    save.await.unwrap().unwrap();
    assert!(matches!(
        clear.await.unwrap().unwrap(),
        AuthClearOutcome::Cleared
    ));

    let snapshot = backend.snapshot();
    assert_eq!(snapshot.save_calls, 1);
    assert_eq!(snapshot.clear_calls, 1);
    assert!(snapshot.secret.is_none());
    assert!(auth_state.lock().unwrap().token.is_none());
}

#[cfg(feature = "app")]
#[tokio::test]
async fn stale_save_after_logout_is_never_committed() {
    let backend = Arc::new(DelayedCredentialStore::default());
    let store = TwitchAuthStore::with_backend(backend.clone());
    let auth_state = Arc::new(Mutex::new(twitch_auth_state()));
    let (generation, snapshot) = {
        let mut auth = auth_state.lock().unwrap();
        let snapshot = auth.clone();
        let generation = auth.generation;

        // This is the in-memory portion of logout. It must complete before
        // waiting for credential I/O so a save queued from an older auth
        // operation can observe that it is stale.
        auth.invalidate_operations();
        auth.token = None;
        auth.profile = None;
        (generation, snapshot)
    };

    assert!(matches!(
        store
            .save_if_current(auth_state.clone(), generation, snapshot)
            .await
            .unwrap(),
        AuthSaveOutcome::Stale
    ));
    assert_eq!(backend.snapshot().save_calls, 0);
    assert!(auth_state.lock().unwrap().token.is_none());
}

#[cfg(feature = "app")]
#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn stale_clear_after_newer_auth_never_deletes_its_durable_credential() {
    let backend = Arc::new(DelayedCredentialStore::default());
    let store = TwitchAuthStore::with_backend(backend.clone());
    let auth_state = Arc::new(Mutex::new(twitch_auth_state()));
    let old_generation = auth_state.lock().unwrap().generation;

    // Keep an already-authorized old save inside the credential backend so
    // both logout clear and the newer save must contend for the real I/O
    // lock rather than relying on task scheduling order.
    let old_save = tokio::spawn({
        let store = store.clone();
        let auth_state = auth_state.clone();
        async move {
            store
                .save_if_current(auth_state, old_generation, twitch_auth_state())
                .await
        }
    });
    backend.wait_until_save_started();

    let logout_generation = {
        let mut auth = auth_state.lock().unwrap();
        let generation = auth.invalidate_operations();
        auth.token = None;
        auth.profile = None;
        generation
    };
    let clear = tokio::spawn({
        let store = store.clone();
        let auth_state = auth_state.clone();
        async move { store.clear_if_current(auth_state, logout_generation).await }
    });

    let (newer_generation, newer_auth) = {
        let mut auth = auth_state.lock().unwrap();
        let generation = auth.invalidate_operations();
        auth.replace_token(
            TokenResponse {
                access_token: "newer-access-token".to_string(),
                refresh_token: "newer-refresh-token".to_string(),
                scope: vec!["user:read:chat".to_string()],
                expires_in: 7200,
            },
            TwitchUserProfile {
                user_id: "newer-user-id".to_string(),
                login: "newer-viewer".to_string(),
                client_id: "client-id".to_string(),
                scopes: vec!["user:read:chat".to_string()],
                expires_in: 7200,
            },
        )
        .unwrap();
        (generation, auth.clone())
    };
    let newer_save = tokio::spawn({
        let store = store.clone();
        let auth_state = auth_state.clone();
        async move {
            store
                .save_if_current(auth_state, newer_generation, newer_auth)
                .await
        }
    });

    backend.release_save();
    assert!(matches!(
        old_save.await.unwrap().unwrap(),
        AuthSaveOutcome::Saved(None)
    ));
    assert!(matches!(
        clear.await.unwrap().unwrap(),
        AuthClearOutcome::Stale
    ));
    assert!(matches!(
        newer_save.await.unwrap().unwrap(),
        AuthSaveOutcome::Saved(None)
    ));

    let persisted =
        serde_json::from_str::<StoredTwitchAuth>(backend.snapshot().secret.as_deref().unwrap())
            .unwrap();
    assert_eq!(persisted.access_token, "newer-access-token");
    assert_eq!(backend.snapshot().clear_calls, 0);
}

#[cfg(feature = "app")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delayed_failed_logout_clear_keeps_newer_authentication() {
    let backend = Arc::new(DelayedCredentialStore::default());
    backend.delay_and_fail_clear();
    let store = TwitchAuthStore::with_backend(backend.clone());
    let auth_state = Arc::new(Mutex::new(twitch_auth_state()));

    let clear = tokio::spawn({
        let store = store.clone();
        let auth_state = auth_state.clone();
        async move { clear_twitch_auth_state_with_store(auth_state, &store).await }
    });
    backend.wait_until_clear_started();

    // The command has invalidated the old generation but must not retain the
    // auth mutex while a slow keyring clear is pending. Complete a newer
    // authentication while that durable operation is blocked.
    let newer_generation = {
        let mut auth = auth_state
            .try_lock()
            .expect("delayed credential clear must release the auth mutex");
        let generation = auth.invalidate_operations();
        auth.replace_token(
            TokenResponse {
                access_token: "newer-access-token".to_string(),
                refresh_token: "newer-refresh-token".to_string(),
                scope: vec!["user:read:chat".to_string()],
                expires_in: 7200,
            },
            TwitchUserProfile {
                user_id: "newer-user-id".to_string(),
                login: "newer-viewer".to_string(),
                client_id: "client-id".to_string(),
                scopes: vec!["user:read:chat".to_string()],
                expires_in: 7200,
            },
        )
        .unwrap();
        generation
    };

    backend.release_clear();
    let error = clear.await.unwrap().unwrap_err();

    assert!(error.contains("資格情報ストア"));
    let auth = auth_state.lock().unwrap();
    assert_eq!(auth.generation, newer_generation);
    let credentials = auth.eventsub_credentials().unwrap();
    assert_eq!(credentials.access_token, "newer-access-token");
    assert_eq!(credentials.refresh_token, "newer-refresh-token");
}

#[cfg(feature = "app")]
#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn delayed_successful_logout_clear_preserves_newer_authentication_and_connection() {
    let backend = Arc::new(DelayedCredentialStore::default());
    backend.delay_clear();
    let store = TwitchAuthStore::with_backend(backend.clone());
    let auth_state = Arc::new(Mutex::new(twitch_auth_state()));

    let clear = tokio::spawn({
        let store = store.clone();
        let auth_state = auth_state.clone();
        async move { clear_twitch_auth_state_with_store(auth_state, &store).await }
    });
    backend.wait_until_clear_started();

    let (newer_generation, newer_auth) = {
        let mut auth = auth_state
            .try_lock()
            .expect("delayed credential clear must release the auth mutex");
        let generation = auth.invalidate_operations();
        auth.replace_token(
            TokenResponse {
                access_token: "newer-access-token".to_string(),
                refresh_token: "newer-refresh-token".to_string(),
                scope: vec!["user:read:chat".to_string()],
                expires_in: 7200,
            },
            TwitchUserProfile {
                user_id: "newer-user-id".to_string(),
                login: "newer-viewer".to_string(),
                client_id: "client-id".to_string(),
                scopes: vec!["user:read:chat".to_string()],
                expires_in: 7200,
            },
        )
        .unwrap();
        (generation, auth.clone())
    };
    let newer_connection = Arc::new(Mutex::new(Some(TwitchConnectionHandle::new(
        newer_generation,
        tokio::spawn(async { std::future::pending::<()>().await }),
    ))));
    let save = tokio::spawn({
        let store = store.clone();
        let auth_state = auth_state.clone();
        async move {
            store
                .save_if_current(auth_state, newer_generation, newer_auth)
                .await
        }
    });

    backend.release_clear();
    let error = clear.await.unwrap().unwrap_err();
    // The real command only takes/aborts its connection after an Ok result.
    // Stale-after-clear therefore leaves this newer connection untouched.
    assert!(error.contains("古い解除結果"));
    assert!(newer_connection.lock().unwrap().is_some());
    backend.release_save();
    assert!(matches!(
        save.await.unwrap().unwrap(),
        AuthSaveOutcome::Saved(None)
    ));

    let persisted =
        serde_json::from_str::<StoredTwitchAuth>(backend.snapshot().secret.as_deref().unwrap())
            .unwrap();
    assert_eq!(persisted.access_token, "newer-access-token");
    newer_connection.lock().unwrap().take().unwrap().abort();
}

#[cfg(feature = "app")]
#[test]
fn failed_logout_recovery_keeps_the_new_generation() {
    let mut auth = twitch_auth_state();
    let stale_generation = auth.generation;
    let previous_auth = auth.clone();
    let logout_generation = auth.invalidate_operations();
    auth.token = None;
    auth.profile = None;

    restore_auth_after_failed_clear_if_current(&mut auth, logout_generation, previous_auth);

    assert_eq!(auth.generation, logout_generation);
    assert_ne!(auth.generation, stale_generation);
    assert!(auth.pending.is_none());
    assert!(auth.token.is_some());
}

#[cfg(feature = "app")]
#[test]
fn newer_auth_generation_invalidates_an_in_flight_poll_and_credentials() {
    let mut auth = twitch_auth_state();
    auth.pending = Some(super::PendingDeviceAuth {
        generation: auth.generation,
        poll_in_flight: true,
        client_id: "client-id".to_string(),
        device_code: "device-a".to_string(),
        interval: 5,
    });
    let stale_generation = auth.generation;

    let current_generation = auth.invalidate_operations();
    auth.token = None;
    auth.profile = None;

    assert_ne!(stale_generation, current_generation);
    assert!(!auth.pending_is_current(stale_generation));
    assert!(auth.pending.is_none());
    assert!(auth.token.is_none());
    assert!(auth.profile.is_none());
}

fn scopes_without_chat_read() -> Vec<String> {
    vec!["user:read:email".to_string()]
}

#[test]
fn initial_authorization_requires_chat_read_scope() {
    let error = ensure_required_twitch_scopes(&scopes_without_chat_read()).unwrap_err();

    assert!(error.to_string().contains("user:read:chat"));
    assert!(error.to_string().contains("再ログイン"));
}

#[cfg(feature = "app")]
#[test]
fn stored_auth_without_chat_read_scope_is_not_restored() {
    let secret = serde_json::to_string(&StoredTwitchAuth {
        client_id: "client-id".to_string(),
        access_token: "access-token".to_string(),
        refresh_token: "refresh-token".to_string(),
        scopes: scopes_without_chat_read(),
        expires_in: 3600,
        profile: TwitchUserProfile {
            user_id: "user-id".to_string(),
            login: "viewer".to_string(),
            client_id: "client-id".to_string(),
            scopes: scopes_without_chat_read(),
            expires_in: 3600,
        },
    })
    .unwrap();

    let error = restore_stored_auth(&secret).unwrap_err();

    assert!(error.to_string().contains("user:read:chat"));
}

#[test]
fn refreshed_authorization_requires_chat_read_scope() {
    let error = ensure_required_twitch_scopes(&scopes_without_chat_read()).unwrap_err();

    assert!(error.to_string().contains("user:read:chat"));
}

#[cfg(feature = "app")]
#[test]
fn eventsub_credentials_reject_missing_chat_read_scope_before_subscription() {
    let mut auth = twitch_auth_state();
    auth.profile.as_mut().unwrap().scopes = scopes_without_chat_read();

    let error = auth.eventsub_credentials().unwrap_err();

    assert!(error.to_string().contains("user:read:chat"));
}

fn process_session(
    dedupe: &mut MessageDedupe,
    message_ids: &[&str],
    received_at: Instant,
) -> Vec<bool> {
    message_ids
        .iter()
        .map(|id| dedupe.insert_at((*id).to_string(), received_at))
        .collect()
}

#[test]
fn reads_device_flow_status_from_twitch_message_field() {
    let response = serde_json::from_str::<OAuthErrorResponse>(
        r#"{"status":400,"message":"authorization_pending"}"#,
    )
    .unwrap();

    assert_eq!(oauth_error_code(&response), Some("authorization_pending"));
}

#[test]
fn reads_standard_oauth_error_field_when_present() {
    let response = serde_json::from_str::<OAuthErrorResponse>(
        r#"{"error":"slow_down","message":"wait before polling again"}"#,
    )
    .unwrap();

    assert_eq!(oauth_error_code(&response), Some("slow_down"));
}

fn utc_timestamp(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .with_timezone(&Utc)
}

fn chat_fixture_with_timestamp(timestamp: Option<serde_json::Value>) -> EventSubEnvelope {
    let mut fixture = serde_json::from_str::<serde_json::Value>(include_str!(
        "fixtures/channel_chat_message.json"
    ))
    .unwrap();
    let metadata = fixture["metadata"].as_object_mut().unwrap();
    match timestamp {
        Some(timestamp) => {
            metadata.insert("message_timestamp".to_string(), timestamp);
        }
        None => {
            metadata.remove("message_timestamp");
        }
    }
    serde_json::from_value(fixture).unwrap()
}

#[test]
fn parses_channel_chat_message_fixture() {
    let fixture = include_str!("fixtures/channel_chat_message.json");
    let envelope = serde_json::from_str::<EventSubEnvelope>(fixture).unwrap();
    let normalized = normalize_chat_message(envelope, utc_timestamp("2026-08-15T12:34:56.789Z"), 0)
        .unwrap()
        .unwrap();
    assert!(normalized.timestamp_warning.is_none());
    let message = normalized.message;

    assert_eq!(message.id, "cc106a89-1814-919d-454c-f4f2f970aae7");
    assert_eq!(message.channel_id, "1971641");
    assert_eq!(message.channel_login, "streamer");
    assert_eq!(message.user_id, "4145994");
    assert_eq!(message.user_login, "viewer32");
    assert_eq!(message.user_display_name, "viewer32");
    assert_eq!(message.text, "Hi chat Kappa");
    assert_eq!(message.fragments.len(), 2);
    assert_eq!(message.fragments[1].kind, "emote");
    assert_eq!(message.fragments[1].emote.as_ref().unwrap().id, "25");
    assert_eq!(message.badges[0].set_id, "broadcaster");
    assert_eq!(
        message.received_at,
        utc_timestamp("2023-11-06T18:11:47.492253549Z")
    );
}

#[test]
fn normalized_chat_message_owns_the_connection_generation_before_serialization() {
    let envelope = serde_json::from_str::<EventSubEnvelope>(include_str!(
        "fixtures/channel_chat_message.json"
    ))
    .unwrap();
    let normalized =
        normalize_chat_message(envelope, utc_timestamp("2026-08-15T12:34:56.789Z"), 42)
            .unwrap()
            .unwrap();

    assert_eq!(normalized.message.connection_generation, Some(42));
    assert!(normalized.message.belongs_to_connection_generation(42));
    assert!(!normalized.message.belongs_to_connection_generation(43));
    assert_eq!(
        serde_json::to_value(&normalized.message).unwrap()["connectionGeneration"],
        serde_json::json!(42)
    );
}

#[test]
fn bridge_option_fields_are_omitted_and_auth_warning_is_camel_case() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../src/tauri/fixtures/bridge-contract.json"
    ))
    .unwrap();
    let expected = &fixture["optionOmissions"];
    let fragment = MessageFragment {
        kind: "text".into(),
        text: "hello".into(),
        emote: None,
        cheermote: None,
    };
    let emote = ChatEmote {
        id: "25".into(),
        emote_set_id: "0".into(),
        owner_id: None,
    };
    let profile = TwitchUserProfile {
        user_id: "user-id".into(),
        login: "viewer".into(),
        client_id: "client-id".into(),
        scopes: vec![CHAT_READ_SCOPE.into()],
        expires_in: 3600,
    };
    let authorized = TwitchAuthPollResult::Authorized {
        profile: profile.clone(),
        storage_warning: Some("認証情報は今回の起動中だけ有効です。".into()),
    };
    let validation = TwitchAuthValidationResult {
        profile,
        storage_warning: None,
    };

    let fragment = serde_json::to_value(fragment).unwrap();
    let emote = serde_json::to_value(emote).unwrap();
    let authorized = serde_json::to_value(authorized).unwrap();
    let validation = serde_json::to_value(validation).unwrap();

    assert_eq!(fragment, expected["fragment"]);
    assert_eq!(emote, expected["emote"]);
    assert_eq!(authorized, fixture["authorizedPoll"]);
    assert_eq!(validation, expected["authValidation"]);
}

#[test]
fn normalizes_offset_timestamp_to_utc_and_serializes_the_tauri_field_contract() {
    let normalized = normalize_chat_message(
        chat_fixture_with_timestamp(Some(serde_json::json!(
            "2026-08-15T21:34:56.789123456+09:00"
        ))),
        utc_timestamp("2026-08-15T00:00:00Z"),
        0,
    )
    .unwrap()
    .unwrap();

    assert!(normalized.timestamp_warning.is_none());
    assert_eq!(
        normalized.message.received_at,
        utc_timestamp("2026-08-15T12:34:56.789123456Z")
    );
    let serialized = serde_json::to_value(&normalized.message).unwrap();
    assert_eq!(
        serialized["receivedAt"],
        serde_json::json!("2026-08-15T12:34:56.789123456Z")
    );
    assert!(serialized.get("received_at").is_none());
}

#[test]
fn falls_back_to_websocket_receive_time_for_unsupported_timestamps() {
    let fallback = utc_timestamp("2026-08-15T12:34:56.789Z");
    let cases = [
        ("missing", None),
        ("empty", Some(serde_json::json!(""))),
        ("naive", Some(serde_json::json!("2026-08-15T12:34:56"))),
        ("invalid", Some(serde_json::json!("invalid-timestamp"))),
        ("non-string", Some(serde_json::json!({ "seconds": 1 }))),
        (
            "leap second",
            Some(serde_json::json!("2016-12-31T23:59:60Z")),
        ),
    ];

    for (case_name, timestamp) in cases {
        let normalized =
            normalize_chat_message(chat_fixture_with_timestamp(timestamp), fallback, 0)
                .unwrap()
                .unwrap();

        assert_eq!(normalized.message.received_at, fallback, "{case_name}");
        assert!(
            normalized
                .timestamp_warning
                .as_deref()
                .is_some_and(|warning| warning.contains("WebSocket 受信時刻")),
            "{case_name}"
        );
    }
}

#[test]
fn twitch_transport_deadlines_are_bounded() {
    assert_eq!(TWITCH_HTTP_TIMEOUT, Duration::from_secs(15));
    #[cfg(feature = "app")]
    assert_eq!(TWITCH_WS_HANDSHAKE_TIMEOUT, Duration::from_secs(10));
}

#[test]
fn typed_http_and_oauth_errors_determine_auth_and_retry_policy() {
    let unauthorized = anyhow::Error::new(TwitchApiError::Http {
        status: 401,
        code: None,
        message: "invalid access token".to_string(),
    });
    let invalid_grant = anyhow::Error::new(TwitchApiError::Http {
        status: 400,
        code: Some("invalid_grant".to_string()),
        message: "refresh token is invalid".to_string(),
    });
    let bad_condition = TwitchApiError::Http {
        status: 400,
        code: None,
        message: "condition is invalid".to_string(),
    };
    let missing_scope = TwitchApiError::Http {
        status: 403,
        code: None,
        message: "missing scope".to_string(),
    };
    let unavailable = TwitchApiError::Http {
        status: 503,
        code: None,
        message: "unavailable".to_string(),
    };

    assert!(is_definitive_auth_failure(&unauthorized));
    assert!(is_definitive_auth_failure(&invalid_grant));
    assert_eq!(
        bad_condition.auth_failure(),
        None,
        "a 400 API condition error must not masquerade as an auth failure"
    );
    assert_eq!(
        missing_scope.auth_failure(),
        None,
        "a 403 permission error must remain distinct from a token failure"
    );
    assert_eq!(
        TwitchApiError::Http {
            status: 401,
            code: Some("ignored-for-401".to_string()),
            message: "invalid access token".to_string(),
        }
        .auth_failure(),
        Some(TwitchAuthFailure::InvalidAccessToken)
    );
    assert_eq!(
        TwitchApiError::Http {
            status: 400,
            code: Some("invalid_grant".to_string()),
            message: "refresh token is invalid".to_string(),
        }
        .auth_failure(),
        Some(TwitchAuthFailure::InvalidGrant)
    );
    assert!(!bad_condition.is_transient());
    assert!(!missing_scope.is_transient());
    assert!(unavailable.is_transient());
}

#[test]
fn message_dedupe_rejects_duplicate_ids() {
    let mut dedupe = MessageDedupe::new(2, Duration::from_secs(60));

    assert!(dedupe.insert("a".to_string()));
    assert!(!dedupe.insert("a".to_string()));
    assert!(dedupe.insert("b".to_string()));
    assert!(dedupe.insert("c".to_string()));
    assert!(dedupe.insert("a".to_string()));
}

#[test]
fn message_dedupe_expires_ids_after_ttl() {
    let started_at = Instant::now();
    let mut dedupe = MessageDedupe::new(10, Duration::from_secs(60));

    assert!(dedupe.insert_at("a".to_string(), started_at));
    assert!(!dedupe.insert_at("a".to_string(), started_at + Duration::from_secs(59)));
    assert!(dedupe.insert_at("a".to_string(), started_at + Duration::from_secs(60)));
}

#[test]
fn normal_reconnect_keeps_message_dedupe() {
    let started_at = Instant::now();
    let mut connection_dedupe = MessageDedupe::new(10, Duration::from_secs(60));

    let first_session = process_session(&mut connection_dedupe, &["before-reconnect"], started_at);
    let reconnected_session = process_session(
        &mut connection_dedupe,
        &["before-reconnect", "after-reconnect"],
        started_at + Duration::from_secs(2),
    );

    assert_eq!(first_session, vec![true]);
    assert_eq!(reconnected_session, vec![false, true]);
}

#[test]
fn reconnect_handover_keeps_message_dedupe() {
    let started_at = Instant::now();
    let mut connection_dedupe = MessageDedupe::new(10, Duration::from_secs(60));

    let original_session =
        process_session(&mut connection_dedupe, &["before-handover"], started_at);
    let handover_session = process_session(
        &mut connection_dedupe,
        &["before-handover", "after-handover"],
        started_at + Duration::from_millis(100),
    );

    assert_eq!(original_session, vec![true]);
    assert_eq!(handover_session, vec![false, true]);
}

#[test]
fn retry_backoff_caps_after_repeated_reconnects() {
    assert_eq!(retry_backoff_seconds(0), 2);
    assert_eq!(retry_backoff_seconds(1), 2);
    assert_eq!(retry_backoff_seconds(2), 5);
    assert_eq!(retry_backoff_seconds(3), 10);
    assert_eq!(retry_backoff_seconds(4), 30);
    assert_eq!(retry_backoff_seconds(10), 30);
}

#[cfg(feature = "app")]
#[test]
fn eventsub_handover_deadline_leaves_time_for_twitch_to_close_the_old_socket() {
    // Twitch grants 30 seconds to connect to the supplied reconnect URL and
    // close the old socket. Keep consuming the old stream until a little
    // before that deadline, then allow the ordinary reconnect supervisor to
    // take over if the new welcome never arrives.
    assert_eq!(EVENTSUB_RECONNECT_HANDOVER_TIMEOUT, Duration::from_secs(25));
    assert!(EVENTSUB_RECONNECT_HANDOVER_TIMEOUT < Duration::from_secs(30));
}

#[cfg(feature = "app")]
#[test]
fn eventsub_backoff_resets_only_after_a_stable_established_session() {
    let started_at = Instant::now();
    let mut backoff = EventSubReconnectBackoff::default();

    assert_eq!(backoff.next_delay_after_failure_at(started_at), 2);
    assert_eq!(
        backoff.next_delay_after_failure_at(started_at + Duration::from_secs(2)),
        5
    );

    // A welcome/subscription that immediately fails must preserve the
    // accumulated delay, so a flaky endpoint cannot cause a retry storm.
    let first_welcome_at = started_at + Duration::from_secs(3);
    backoff.record_session_established_at(first_welcome_at);
    assert_eq!(
        backoff.next_delay_after_failure_at(first_welcome_at + Duration::from_secs(1)),
        10
    );

    // Once a welcome (and, for a normal connection, its subscription) has
    // remained healthy for the stability window, the next fault is a new
    // failure sequence and returns to the shortest retry delay.
    let stable_welcome_at = started_at + Duration::from_secs(10);
    backoff.record_session_established_at(stable_welcome_at);
    assert_eq!(
        backoff.next_delay_after_failure_at(
            stable_welcome_at + EVENTSUB_BACKOFF_RESET_STABLE_DURATION
        ),
        2
    );
}

#[cfg(feature = "app")]
#[test]
fn eventsub_backoff_keeps_growing_without_an_established_session() {
    let started_at = Instant::now();
    let mut backoff = EventSubReconnectBackoff::default();

    assert_eq!(backoff.next_delay_after_failure_at(started_at), 2);
    assert_eq!(
        backoff.next_delay_after_failure_at(started_at + Duration::from_secs(2)),
        5
    );
    assert_eq!(
        backoff.next_delay_after_failure_at(started_at + Duration::from_secs(7)),
        10
    );
    assert_eq!(
        backoff.next_delay_after_failure_at(started_at + Duration::from_secs(17)),
        30
    );
    assert_eq!(
        backoff.next_delay_after_failure_at(started_at + Duration::from_secs(47)),
        30
    );
}

#[cfg(feature = "app")]
#[test]
fn eventsub_handover_preserves_backoff_until_the_new_session_is_stable() {
    let started_at = Instant::now();
    let mut backoff = EventSubReconnectBackoff::default();

    assert_eq!(backoff.next_delay_after_failure_at(started_at), 2);
    assert_eq!(
        backoff.next_delay_after_failure_at(started_at + Duration::from_secs(2)),
        5
    );

    // A server-requested handover itself does not reset the retry budget.
    // Its new welcome is the lifecycle point that can establish a session.
    let handover_welcome_at = started_at + Duration::from_secs(7);
    backoff.record_session_established_at(handover_welcome_at);
    assert_eq!(
        backoff.next_delay_after_failure_at(handover_welcome_at + Duration::from_secs(1)),
        10
    );
}

#[cfg(feature = "app")]
#[test]
fn eventsub_handover_failure_before_new_welcome_ignores_the_old_stable_session() {
    let started_at = Instant::now();
    let mut backoff = EventSubReconnectBackoff::default();

    assert_eq!(backoff.next_delay_after_failure_at(started_at), 2);
    assert_eq!(
        backoff.next_delay_after_failure_at(started_at + Duration::from_secs(2)),
        5
    );

    let old_welcome_at = started_at + Duration::from_secs(3);
    backoff.record_session_established_at(old_welcome_at);
    let handover_started_at = old_welcome_at + EVENTSUB_BACKOFF_RESET_STABLE_DURATION;
    backoff.record_handover_started();

    // A handover connection failure before its welcome must continue the
    // existing failure sequence, even when the old session was stable.
    assert_eq!(
        backoff.next_delay_after_failure_at(handover_started_at + Duration::from_secs(1)),
        10
    );
}

#[cfg(feature = "app")]
#[tokio::test]
async fn eventsub_reconnect_with_expired_access_token_refreshes_and_retries_once() {
    let attempted_tokens = RefCell::new(Vec::new());
    let refresh_calls = RefCell::new(0);

    let result = retry_eventsub_subscription(
        "expired-access-token".to_string(),
        |access_token| {
            attempted_tokens.borrow_mut().push(access_token.clone());
            async move {
                if access_token == "expired-access-token" {
                    Err(SubscriptionRequestError::Unauthorized)
                } else {
                    Ok(())
                }
            }
        },
        || {
            *refresh_calls.borrow_mut() += 1;
            async { Ok("refreshed-access-token".to_string()) }
        },
    )
    .await;

    assert!(result.is_ok());
    assert_eq!(
        *attempted_tokens.borrow(),
        vec!["expired-access-token", "refreshed-access-token"]
    );
    assert_eq!(*refresh_calls.borrow(), 1);
}

#[cfg(feature = "app")]
#[tokio::test]
async fn eventsub_permanent_subscription_errors_do_not_refresh_or_retry() {
    let attempts = RefCell::new(0);
    let refresh_calls = RefCell::new(0);
    let result = retry_eventsub_subscription(
        "access-token".to_string(),
        |_| {
            *attempts.borrow_mut() += 1;
            async {
                Err(SubscriptionRequestError::Permanent(TwitchApiError::Http {
                    status: 400,
                    code: Some("Bad Request".to_string()),
                    message: "condition is invalid".to_string(),
                }))
            }
        },
        || {
            *refresh_calls.borrow_mut() += 1;
            async { Ok("unexpected-refresh".to_string()) }
        },
    )
    .await;

    assert_eq!(*attempts.borrow(), 1);
    assert_eq!(*refresh_calls.borrow(), 0);
    assert!(matches!(
        result,
        Err(SubscriptionRequestError::Permanent(TwitchApiError::Http {
            status: 400,
            ..
        }))
    ));
}

#[cfg(feature = "app")]
#[tokio::test]
async fn eventsub_refresh_failure_does_not_retry_the_subscription() {
    let attempts = RefCell::new(0);
    let result = retry_eventsub_subscription(
        "expired-access-token".to_string(),
        |_| {
            *attempts.borrow_mut() += 1;
            async { Err(SubscriptionRequestError::Unauthorized) }
        },
        || async {
            Err(SubscriptionRequestError::AuthRequired(
                "refresh token was revoked".to_string(),
            ))
        },
    )
    .await;

    assert_eq!(*attempts.borrow(), 1);
    assert!(
        matches!(result, Err(SubscriptionRequestError::AuthRequired(message)) if message.contains("revoked"))
    );
}

#[cfg(feature = "app")]
#[test]
fn eventsub_credentials_use_the_refreshed_token_and_persist_refresh_rotation() {
    let mut auth = twitch_auth_state();
    assert_eq!(
        auth.eventsub_credentials().unwrap().access_token,
        "access-token"
    );

    auth.replace_token(
        TokenResponse {
            access_token: "refreshed-access-token".to_string(),
            refresh_token: "rotated-refresh-token".to_string(),
            scope: vec!["user:read:chat".to_string()],
            expires_in: 7200,
        },
        TwitchUserProfile {
            user_id: "user-id".to_string(),
            login: "viewer".to_string(),
            client_id: "client-id".to_string(),
            scopes: vec!["user:read:chat".to_string()],
            expires_in: 7200,
        },
    )
    .unwrap();

    let stored = auth.stored_auth().unwrap();
    assert_eq!(
        auth.eventsub_credentials().unwrap().access_token,
        "refreshed-access-token"
    );
    assert_eq!(stored.access_token, "refreshed-access-token");
    assert_eq!(stored.refresh_token, "rotated-refresh-token");
    assert_eq!(stored.expires_in, 7200);

    let secure = FakeAuthSecretStore::default();
    let legacy = FakeAuthSecretStore::default();
    let storage = AuthStorage {
        secure: &secure,
        legacy: &legacy,
    };
    assert_eq!(storage.save(&auth).unwrap(), None);
    let persisted =
        serde_json::from_str::<StoredTwitchAuth>(secure.secret.borrow().as_deref().unwrap())
            .unwrap();
    assert_eq!(persisted.refresh_token, "rotated-refresh-token");
}

#[cfg(feature = "app")]
#[test]
fn eventsub_refresh_rejects_a_new_token_without_chat_read_scope() {
    let mut auth = twitch_auth_state();
    let error = auth
        .replace_token(
            TokenResponse {
                access_token: "refreshed-access-token".to_string(),
                refresh_token: "rotated-refresh-token".to_string(),
                // Twitch can omit scope from a refresh response, so this must not
                // fall back to the previously stored profile's scopes.
                scope: Vec::new(),
                expires_in: 7200,
            },
            TwitchUserProfile {
                user_id: "user-id".to_string(),
                login: "viewer".to_string(),
                client_id: "client-id".to_string(),
                scopes: scopes_without_chat_read(),
                expires_in: 7200,
            },
        )
        .unwrap_err();

    assert!(error.to_string().contains("user:read:chat"));
    assert_eq!(
        auth.eventsub_credentials().unwrap().access_token,
        "access-token"
    );
}

#[cfg(feature = "app")]
#[test]
fn stale_eventsub_scope_failure_keeps_rotated_authentication() {
    let mut auth = twitch_auth_state();
    let stale_refresh_token = auth.eventsub_credentials().unwrap().refresh_token;

    // Simulate a second EventSub re-subscription completing its refresh while
    // the first one is awaiting validation of a scope-deficient token.
    auth.replace_token(
        TokenResponse {
            access_token: "newer-access-token".to_string(),
            refresh_token: "newer-refresh-token".to_string(),
            scope: vec!["user:read:chat".to_string()],
            expires_in: 7200,
        },
        TwitchUserProfile {
            user_id: "user-id".to_string(),
            login: "viewer".to_string(),
            client_id: "client-id".to_string(),
            scopes: vec!["user:read:chat".to_string()],
            expires_in: 7200,
        },
    )
    .unwrap();

    let access_token =
        clear_auth_for_eventsub_missing_scope_if_current(&mut auth, &stale_refresh_token).unwrap();

    assert_eq!(access_token.as_deref(), Some("newer-access-token"));
    let current = auth.eventsub_credentials().unwrap();
    assert_eq!(current.access_token, "newer-access-token");
    assert_eq!(current.refresh_token, "newer-refresh-token");
}

#[cfg(feature = "app")]
#[test]
fn saves_auth_to_the_secure_store_when_available() {
    let secure = FakeAuthSecretStore::default();
    let legacy = FakeAuthSecretStore::default();
    let storage = AuthStorage {
        secure: &secure,
        legacy: &legacy,
    };

    assert_eq!(storage.save(&twitch_auth_state()).unwrap(), None);
    assert!(secure.secret.borrow().is_some());
    assert!(legacy.secret.borrow().is_none());
    assert_eq!(*legacy.save_calls.borrow(), 0);
}

#[cfg(feature = "app")]
#[test]
fn keeps_auth_session_only_when_secure_store_write_fails() {
    let secure = FakeAuthSecretStore {
        fail_save: true,
        ..FakeAuthSecretStore::default()
    };
    let legacy = FakeAuthSecretStore::default();
    let storage = AuthStorage {
        secure: &secure,
        legacy: &legacy,
    };

    let warning = storage.save(&twitch_auth_state()).unwrap().unwrap();

    assert!(warning.contains("この起動中だけ有効"));
    assert!(warning.contains("認証情報ファイルは作成していません"));
    assert!(secure.secret.borrow().is_none());
    assert!(legacy.secret.borrow().is_none());
    assert_eq!(*legacy.save_calls.borrow(), 0);
}

#[cfg(feature = "app")]
#[test]
fn migrates_existing_legacy_auth_after_secure_store_recovers() {
    let legacy_secret = stored_auth_secret();
    let secure = FakeAuthSecretStore::default();
    let legacy = FakeAuthSecretStore::with_secret(legacy_secret.clone());
    let storage = AuthStorage {
        secure: &secure,
        legacy: &legacy,
    };

    let restored = storage.load();

    assert_eq!(restored.auth.unwrap().profile().unwrap().login, "viewer");
    assert!(restored
        .storage_warning
        .unwrap()
        .contains("移行し、平文ファイルを削除"));
    assert_eq!(
        secure.secret.borrow().as_deref(),
        Some(legacy_secret.as_str())
    );
    assert!(legacy.secret.borrow().is_none());
}

#[cfg(feature = "app")]
#[test]
fn warns_when_secure_store_cannot_be_read_and_no_legacy_auth_exists() {
    let secure = FakeAuthSecretStore {
        fail_load: true,
        ..FakeAuthSecretStore::default()
    };
    let legacy = FakeAuthSecretStore::default();
    let storage = AuthStorage {
        secure: &secure,
        legacy: &legacy,
    };

    let restored = storage.load();

    assert!(restored.auth.is_none());
    let warning = restored.storage_warning.unwrap();
    assert!(warning.contains("資格情報ストアから Twitch 認証情報を読み込めません"));
    assert!(warning.contains("fake secure-store read failure"));
    assert!(warning.contains("資格情報ストアを確認"));
    assert!(warning.contains("再ログイン"));
}

#[cfg(feature = "app")]
#[test]
fn leaves_legacy_auth_unread_when_migration_is_rejected() {
    let secure = FakeAuthSecretStore {
        fail_save: true,
        ..FakeAuthSecretStore::default()
    };
    let legacy = FakeAuthSecretStore::with_secret(stored_auth_secret());
    let storage = AuthStorage {
        secure: &secure,
        legacy: &legacy,
    };

    let restored = storage.load();

    assert!(restored.auth.is_none());
    let warning = restored.storage_warning.unwrap();
    assert!(warning.contains("安全のため読み込まず"));
    assert!(warning.contains("ファイルを削除"));
    assert!(warning.contains("アクセスを取り消し"));
    assert!(legacy.secret.borrow().is_some());
}

#[cfg(feature = "app")]
#[test]
fn failed_secure_auth_clear_keeps_the_secret_for_retry() {
    let secure = FakeAuthSecretStore {
        fail_clear: true,
        ..FakeAuthSecretStore::with_secret(stored_auth_secret())
    };
    let legacy = FakeAuthSecretStore::default();
    let storage = AuthStorage {
        secure: &secure,
        legacy: &legacy,
    };
    assert!(storage.clear().is_err());
    assert!(secure.secret.borrow().is_some());
}

#[cfg(feature = "app")]
#[test]
fn logout_clears_secure_and_legacy_auth_state() {
    let secure = FakeAuthSecretStore::with_secret(stored_auth_secret());
    let legacy = FakeAuthSecretStore::with_secret(stored_auth_secret());
    let storage = AuthStorage {
        secure: &secure,
        legacy: &legacy,
    };

    storage.clear().unwrap();

    assert!(secure.secret.borrow().is_none());
    assert!(legacy.secret.borrow().is_none());
}
