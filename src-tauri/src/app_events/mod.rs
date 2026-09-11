#[cfg(feature = "app")]
use crate::twitch::ChatMessage;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(feature = "app")]
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub const TWITCH_STATUS_EVENT: &str = "twitch://status";
#[allow(dead_code)]
pub const TWITCH_CHAT_MESSAGE_EVENT: &str = "twitch://chat-message";
pub const SPEECH_QUEUE_UPDATED_EVENT: &str = "speech://queue-updated";
pub const SPEECH_STATUS_EVENT: &str = "speech://status";
pub const APP_LOG_EVENT: &str = "app://log";

const OPERATIONAL_LOG_LIMIT: usize = 500;
const EMIT_ERROR_LIMIT: usize = 100;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLogEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub level: AppLogLevel,
    pub message: String,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AppLogLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchStatusEvent {
    pub revision: u64,
    pub domain: TwitchStatusDomain,
    pub status: TwitchStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<TwitchAuthRequiredReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_generation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_connection: Option<TwitchActiveConnection>,
    pub message: Option<String>,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TwitchActiveConnection {
    pub generation: u64,
    pub broadcaster_user_id: String,
    pub broadcaster_login: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TwitchStatus {
    Disconnected,
    Connecting,
    Connected,
    /// A restored credential has not completed `/validate` yet.
    Validating,
    #[allow(dead_code)]
    Reconnecting,
    AuthRequired,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(PartialEq, Eq)]
pub enum TwitchStatusDomain {
    Auth,
    Chat,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TwitchAuthRequiredReason {
    MissingRequiredScope,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechStatusEvent {
    pub revision: u64,
    pub status: SpeechStatus,
    pub adapter_health: SpeechAdapterHealth,
    pub message: Option<String>,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpeechStatus {
    Idle,
    Speaking,
    Paused,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpeechAdapterHealth {
    Connected,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechQueueUpdatedEvent {
    pub revision: u64,
    pub queued_count: usize,
    pub items: Vec<SpeechQueueItemEvent>,
    pub phase: SpeechQueuePhase,
    pub warning: Option<String>,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechQueueItemEvent {
    pub id: String,
    pub source_message_id: Option<String>,
    pub user_display_name: String,
    pub text: String,
    pub status: SpeechQueueItemStatus,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpeechQueueItemStatus {
    Queued,
    Speaking,
    Spoken,
    Skipped,
    Blocked,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpeechQueuePhase {
    Idle,
    Speaking,
    Paused,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEventEmitError {
    pub id: String,
    pub event: String,
    pub error: String,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEventsSnapshot {
    pub revision: u64,
    pub logs: Vec<AppLogEvent>,
    pub twitch_statuses: Vec<TwitchStatusEvent>,
    pub speech_status: Option<SpeechStatusEvent>,
    pub emit_errors: Vec<AppEventEmitError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechStateSnapshot {
    pub revision: u64,
    pub status: SpeechStatusEvent,
    pub queue: SpeechQueueUpdatedEvent,
}

#[derive(Debug, Default)]
pub struct AppEventState {
    inner: Mutex<AppEventStateInner>,
}

#[derive(Debug, Default)]
struct AppEventStateInner {
    revision: u64,
    logs: VecDeque<AppLogEvent>,
    twitch_auth_status: Option<TwitchStatusEvent>,
    twitch_chat_status: Option<TwitchStatusEvent>,
    speech_status: Option<SpeechStatusEvent>,
    speech_queue: Option<SpeechQueueUpdatedEvent>,
    emit_errors: VecDeque<AppEventEmitError>,
}

impl AppEventState {
    fn next_revision(inner: &mut AppEventStateInner) -> u64 {
        inner.revision = inner.revision.wrapping_add(1).max(1);
        inner.revision
    }

    fn record_log(&self, mut payload: AppLogEvent) -> AppLogEvent {
        let Ok(mut inner) = self.inner.lock() else {
            return payload;
        };
        let revision = Self::next_revision(&mut inner);
        payload.id = Some(format!("log-{revision}"));
        inner.logs.push_front(payload.clone());
        while inner.logs.len() > OPERATIONAL_LOG_LIMIT {
            inner.logs.pop_back();
        }
        payload
    }

    fn record_twitch_status(&self, mut payload: TwitchStatusEvent) -> TwitchStatusEvent {
        let Ok(mut inner) = self.inner.lock() else {
            return payload;
        };
        if payload.domain == TwitchStatusDomain::Chat {
            if let (Some(incoming), Some(current_status)) = (
                payload.connection_generation,
                inner.twitch_chat_status.as_ref(),
            ) {
                if let Some(current) = current_status.connection_generation {
                    if incoming < current {
                        return current_status.clone();
                    }
                    if incoming == current
                        && payload.active_connection.is_none()
                        && !matches!(payload.status, TwitchStatus::Disconnected)
                    {
                        payload.active_connection = current_status.active_connection.clone();
                    }
                }
            }
        }
        payload.revision = Self::next_revision(&mut inner);
        match payload.domain {
            TwitchStatusDomain::Auth => inner.twitch_auth_status = Some(payload.clone()),
            TwitchStatusDomain::Chat => inner.twitch_chat_status = Some(payload.clone()),
        }
        payload
    }

    fn record_speech_status(&self, mut payload: SpeechStatusEvent) -> SpeechStatusEvent {
        let Ok(mut inner) = self.inner.lock() else {
            return payload;
        };
        payload.revision = Self::next_revision(&mut inner);
        inner.speech_status = Some(payload.clone());
        payload
    }

    fn record_speech_queue(&self, mut payload: SpeechQueueUpdatedEvent) -> SpeechQueueUpdatedEvent {
        let Ok(mut inner) = self.inner.lock() else {
            return payload;
        };
        payload.revision = Self::next_revision(&mut inner);
        inner.speech_queue = Some(payload.clone());
        payload
    }

    fn record_emit_error(&self, event: &str, error: String) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        let revision = Self::next_revision(&mut inner);
        inner.emit_errors.push_front(AppEventEmitError {
            id: format!("emit-error-{revision}"),
            event: event.to_string(),
            error,
            occurred_at_ms: current_timestamp_ms(),
        });
        while inner.emit_errors.len() > EMIT_ERROR_LIMIT {
            inner.emit_errors.pop_back();
        }
    }

    pub fn snapshot(&self) -> AppEventsSnapshot {
        let inner = self.inner.lock().expect("app event mutex poisoned");
        AppEventsSnapshot {
            revision: inner.revision,
            logs: inner.logs.iter().cloned().collect(),
            twitch_statuses: [
                inner.twitch_auth_status.clone(),
                inner.twitch_chat_status.clone(),
            ]
            .into_iter()
            .flatten()
            .collect(),
            speech_status: inner.speech_status.clone(),
            emit_errors: inner.emit_errors.iter().cloned().collect(),
        }
    }

    pub fn speech_state_snapshot(&self) -> Option<SpeechStateSnapshot> {
        let inner = self.inner.lock().expect("app event mutex poisoned");
        Some(SpeechStateSnapshot {
            revision: inner.revision,
            status: inner.speech_status.clone()?,
            queue: inner.speech_queue.clone()?,
        })
    }
}

#[cfg(feature = "app")]
fn app_event_state<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::State<'_, AppEventState>> {
    app.try_state::<AppEventState>()
}

#[cfg(feature = "app")]
fn emit_payload<R: Runtime, P: Serialize + Clone>(app: &AppHandle<R>, event: &str, payload: P) {
    if let Err(error) = app.emit(event, payload) {
        let message = error.to_string();
        if let Some(state) = app_event_state(app) {
            state.record_emit_error(event, message.clone());
        }
        eprintln!("failed to emit {event}: {message}");
    }
}

#[cfg(feature = "app")]
pub fn emit_app_log<R: Runtime>(
    app: &AppHandle<R>,
    level: AppLogLevel,
    message: impl Into<String>,
) {
    let payload = AppLogEvent {
        id: None,
        level,
        message: message.into(),
        occurred_at_ms: current_timestamp_ms(),
    };
    if let Some(state) = app_event_state(app) {
        let payload = state.record_log(payload);
        emit_payload(app, APP_LOG_EVENT, payload);
    } else {
        emit_payload(app, APP_LOG_EVENT, payload);
    }
}

#[cfg(feature = "app")]
pub fn emit_twitch_status<R: Runtime>(
    app: &AppHandle<R>,
    domain: TwitchStatusDomain,
    status: TwitchStatus,
    message: Option<String>,
) {
    let payload = TwitchStatusEvent {
        revision: 0,
        domain,
        status,
        reason: None,
        connection_generation: None,
        active_connection: None,
        message,
        occurred_at_ms: current_timestamp_ms(),
    };
    if let Some(state) = app_event_state(app) {
        let payload = state.record_twitch_status(payload);
        emit_payload(app, TWITCH_STATUS_EVENT, payload);
    } else {
        emit_payload(app, TWITCH_STATUS_EVENT, payload);
    }
}

#[cfg(feature = "app")]
pub fn emit_twitch_chat_status<R: Runtime>(
    app: &AppHandle<R>,
    status: TwitchStatus,
    message: Option<String>,
    connection_generation: u64,
    active_connection: Option<TwitchActiveConnection>,
) {
    let payload = TwitchStatusEvent {
        revision: 0,
        domain: TwitchStatusDomain::Chat,
        status,
        reason: None,
        connection_generation: Some(connection_generation),
        active_connection,
        message,
        occurred_at_ms: current_timestamp_ms(),
    };
    if let Some(state) = app_event_state(app) {
        let payload = state.record_twitch_status(payload);
        emit_payload(app, TWITCH_STATUS_EVENT, payload);
    } else {
        emit_payload(app, TWITCH_STATUS_EVENT, payload);
    }
}

#[cfg(feature = "app")]
pub fn emit_twitch_auth_required<R: Runtime>(
    app: &AppHandle<R>,
    reason: TwitchAuthRequiredReason,
    message: impl Into<String>,
) {
    let payload = TwitchStatusEvent {
        revision: 0,
        domain: TwitchStatusDomain::Auth,
        status: TwitchStatus::AuthRequired,
        reason: Some(reason),
        connection_generation: None,
        active_connection: None,
        message: Some(message.into()),
        occurred_at_ms: current_timestamp_ms(),
    };
    if let Some(state) = app_event_state(app) {
        let payload = state.record_twitch_status(payload);
        emit_payload(app, TWITCH_STATUS_EVENT, payload);
    } else {
        emit_payload(app, TWITCH_STATUS_EVENT, payload);
    }
}

#[cfg(feature = "app")]
pub fn emit_twitch_chat_message<R: Runtime>(
    app: &AppHandle<R>,
    mut message: ChatMessage,
    connection_generation: u64,
) {
    message.connection_generation = Some(connection_generation);
    emit_payload(app, TWITCH_CHAT_MESSAGE_EVENT, message);
}

#[cfg(feature = "app")]
pub fn emit_speech_status<R: Runtime>(
    app: &AppHandle<R>,
    status: SpeechStatus,
    message: Option<String>,
) {
    let adapter_health = match status {
        SpeechStatus::Disconnected => SpeechAdapterHealth::Disconnected,
        SpeechStatus::Error => SpeechAdapterHealth::Error,
        SpeechStatus::Idle | SpeechStatus::Speaking | SpeechStatus::Paused => {
            SpeechAdapterHealth::Connected
        }
    };
    let payload = SpeechStatusEvent {
        revision: 0,
        status,
        adapter_health,
        message,
        occurred_at_ms: current_timestamp_ms(),
    };
    if let Some(state) = app_event_state(app) {
        let payload = state.record_speech_status(payload);
        emit_payload(app, SPEECH_STATUS_EVENT, payload);
    } else {
        emit_payload(app, SPEECH_STATUS_EVENT, payload);
    }
}

#[cfg(feature = "app")]
pub fn emit_speech_queue_updated<R: Runtime>(
    app: &AppHandle<R>,
    queued_count: usize,
    items: Vec<SpeechQueueItemEvent>,
    phase: SpeechQueuePhase,
    warning: Option<String>,
) {
    let payload = SpeechQueueUpdatedEvent {
        revision: 0,
        queued_count,
        items,
        phase,
        warning,
        occurred_at_ms: current_timestamp_ms(),
    };
    let payload = if let Some(state) = app_event_state(app) {
        state.record_speech_queue(payload)
    } else {
        payload
    };
    emit_payload(app, SPEECH_QUEUE_UPDATED_EVENT, payload);
}

fn current_timestamp_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as u64,
        Err(_) => 0,
    }
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn app_events_snapshot(state: tauri::State<'_, AppEventState>) -> AppEventsSnapshot {
    state.snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_twitch_status_domain_as_camel_case() {
        let auth = TwitchStatusEvent {
            revision: 1,
            domain: TwitchStatusDomain::Auth,
            status: TwitchStatus::Connected,
            reason: None,
            connection_generation: None,
            active_connection: None,
            message: None,
            occurred_at_ms: 1,
        };
        let chat = TwitchStatusEvent {
            revision: 2,
            domain: TwitchStatusDomain::Chat,
            status: TwitchStatus::Reconnecting,
            reason: None,
            connection_generation: Some(4),
            active_connection: Some(TwitchActiveConnection {
                generation: 4,
                broadcaster_user_id: "123".into(),
                broadcaster_login: "rice".into(),
            }),
            message: None,
            occurred_at_ms: 1,
        };

        assert_eq!(serde_json::to_value(auth).unwrap()["domain"], "auth");
        let chat = serde_json::to_value(chat).unwrap();
        assert_eq!(chat["domain"], "chat");
        assert_eq!(chat["connectionGeneration"], 4);
        assert_eq!(chat["activeConnection"]["broadcasterLogin"], "rice");
    }

    #[test]
    fn chat_status_replay_rejects_an_older_connection_generation() {
        let state = AppEventState::default();
        let current = state.record_twitch_status(TwitchStatusEvent {
            revision: 0,
            domain: TwitchStatusDomain::Chat,
            status: TwitchStatus::Connected,
            reason: None,
            connection_generation: Some(2),
            active_connection: Some(TwitchActiveConnection {
                generation: 2,
                broadcaster_user_id: "b".into(),
                broadcaster_login: "channel_b".into(),
            }),
            message: None,
            occurred_at_ms: 2,
        });
        let stale = state.record_twitch_status(TwitchStatusEvent {
            revision: 0,
            domain: TwitchStatusDomain::Chat,
            status: TwitchStatus::Connected,
            reason: None,
            connection_generation: Some(1),
            active_connection: Some(TwitchActiveConnection {
                generation: 1,
                broadcaster_user_id: "a".into(),
                broadcaster_login: "channel_a".into(),
            }),
            message: None,
            occurred_at_ms: 3,
        });

        assert_eq!(stale.revision, current.revision);
        let reconnecting = state.record_twitch_status(TwitchStatusEvent {
            revision: 0,
            domain: TwitchStatusDomain::Chat,
            status: TwitchStatus::Reconnecting,
            reason: None,
            connection_generation: Some(2),
            active_connection: None,
            message: None,
            occurred_at_ms: 4,
        });
        assert_eq!(
            reconnecting
                .active_connection
                .as_ref()
                .unwrap()
                .broadcaster_login,
            "channel_b"
        );
        assert_eq!(
            state.snapshot().twitch_statuses[0]
                .active_connection
                .as_ref()
                .unwrap()
                .broadcaster_login,
            "channel_b"
        );
    }
    fn log(message: &str) -> AppLogEvent {
        AppLogEvent {
            id: None,
            level: AppLogLevel::Info,
            message: message.into(),
            occurred_at_ms: 1,
        }
    }
    fn status(status: SpeechStatus) -> SpeechStatusEvent {
        SpeechStatusEvent {
            revision: 0,
            status,
            adapter_health: SpeechAdapterHealth::Connected,
            message: None,
            occurred_at_ms: 1,
        }
    }
    fn queue() -> SpeechQueueUpdatedEvent {
        SpeechQueueUpdatedEvent {
            revision: 0,
            queued_count: 1,
            items: vec![SpeechQueueItemEvent {
                id: "q1".into(),
                source_message_id: Some("chat1".into()),
                user_display_name: "viewer".into(),
                text: "hello".into(),
                status: SpeechQueueItemStatus::Queued,
            }],
            phase: SpeechQueuePhase::Paused,
            warning: None,
            occurred_at_ms: 1,
        }
    }
    #[test]
    fn startup_logs_and_unvalidated_auth_are_replayable_and_bounded() {
        let state = AppEventState::default();
        for n in 0..OPERATIONAL_LOG_LIMIT + 2 {
            state.record_log(log(&n.to_string()));
        }
        let event = state.record_twitch_status(TwitchStatusEvent {
            revision: 0,
            domain: TwitchStatusDomain::Auth,
            status: TwitchStatus::Validating,
            reason: None,
            connection_generation: None,
            active_connection: None,
            message: None,
            occurred_at_ms: 1,
        });
        let snapshot = state.snapshot();
        assert_eq!(snapshot.logs.len(), OPERATIONAL_LOG_LIMIT);
        assert_eq!(snapshot.logs.last().unwrap().message, "2");
        assert_ne!(snapshot.logs[0].id, snapshot.logs[1].id);
        assert_eq!(snapshot.twitch_statuses[0].revision, event.revision);
        assert_eq!(
            serde_json::to_value(&snapshot).unwrap()["twitchStatuses"][0]["status"],
            "validating"
        );
    }
    #[test]
    fn paused_state_snapshot_is_stable_without_new_events() {
        let state = AppEventState::default();
        assert!(state.speech_state_snapshot().is_none());
        let status = state.record_speech_status(status(SpeechStatus::Paused));
        let queue = state.record_speech_queue(queue());
        let first = state.speech_state_snapshot().unwrap();
        let second = state.speech_state_snapshot().unwrap();
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(second).unwrap()
        );
        assert_eq!(first.status.revision, status.revision);
        assert_eq!(first.queue.revision, queue.revision);
        assert_eq!(first.queue.items.len(), 1);
        assert_eq!(first.queue.phase, SpeechQueuePhase::Paused);
        assert_eq!(first.status.adapter_health, SpeechAdapterHealth::Connected);
        // Each component keeps its own revision even after unrelated log events.
        state.record_log(log("later"));
        let later = state.speech_state_snapshot().unwrap();
        assert!(later.revision > first.revision);
        assert_eq!(later.queue.revision, first.queue.revision);
    }
    #[test]
    fn snapshot_keeps_exact_payload_revision_pairs_across_updates() {
        let state = AppEventState::default();
        state.record_speech_status(status(SpeechStatus::Paused));
        state.record_speech_queue(queue());
        let old = state.speech_state_snapshot().unwrap();
        let new = state.record_speech_queue(SpeechQueueUpdatedEvent {
            items: vec![],
            queued_count: 0,
            phase: SpeechQueuePhase::Idle,
            ..queue()
        });
        let current = state.speech_state_snapshot().unwrap();
        assert!(new.revision > old.queue.revision);
        assert_eq!(old.queue.items.len(), 1);
        assert_eq!(current.queue.revision, new.revision);
        assert!(current.queue.items.is_empty());
        assert_eq!(current.status.revision, old.status.revision);
    }
    #[test]
    fn emit_errors_are_bounded_and_have_replay_ids() {
        let state = AppEventState::default();
        for _ in 0..EMIT_ERROR_LIMIT + 2 {
            state.record_emit_error(APP_LOG_EVENT, "serialize failed".into());
        }
        let snapshot = state.snapshot();
        assert_eq!(snapshot.emit_errors.len(), EMIT_ERROR_LIMIT);
        assert_ne!(snapshot.emit_errors[0].id, snapshot.emit_errors[1].id);
        assert_eq!(snapshot.emit_errors[0].event, APP_LOG_EVENT);
    }
}
