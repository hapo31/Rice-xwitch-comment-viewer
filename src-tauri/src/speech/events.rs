use super::queue::{SpeechQueueItem, SpeechQueueState, DEFAULT_HISTORY_LIMIT, DEFAULT_QUEUE_LIMIT};
use super::worker;
#[cfg(feature = "app")]
use crate::app_events::{
    emit_app_log, emit_speech_adapter_health, emit_speech_queue_updated, emit_speech_status,
    AppLogLevel,
};
use crate::app_events::{
    SpeechQueueItemEvent, SpeechQueueItemStatus, SpeechQueuePhase, SpeechStatus,
};

#[cfg(feature = "app")]
pub(super) struct TauriSpeechQueueEvents(pub(super) tauri::AppHandle<tauri::Wry>);
#[cfg(feature = "app")]
impl worker::SpeechQueueEvents for TauriSpeechQueueEvents {
    fn snapshot(&self, queue: &SpeechQueueState, warning: Option<String>) {
        emit_queue_snapshot(&self.0, queue, warning);
    }
    fn activity(&self, status: SpeechStatus, message: Option<String>) {
        emit_speech_status(&self.0, status, message);
    }
    fn health(&self, health: crate::app_events::SpeechAdapterHealth, message: Option<String>) {
        emit_speech_adapter_health(&self.0, health, message);
    }
    fn log(&self, level: AppLogLevel, message: String) {
        emit_app_log(&self.0, level, message);
    }
}

#[cfg(feature = "app")]
pub(super) fn emit_queue_snapshot(
    app: &tauri::AppHandle<tauri::Wry>,
    queue: &SpeechQueueState,
    warning: Option<String>,
) {
    let payload = queue_event_snapshot(queue, warning);
    emit_speech_queue_updated(
        app,
        payload.queued_count,
        payload.items,
        payload.phase,
        payload.warning,
    );
}

#[cfg(any(feature = "app", test))]
pub(crate) fn queue_event_snapshot(
    queue: &SpeechQueueState,
    warning: Option<String>,
) -> crate::app_events::SpeechQueueUpdatedEvent {
    let items = queue
        .in_flight
        .iter()
        .chain(queue.pending.iter())
        .chain(queue.history.iter().rev())
        .take(DEFAULT_QUEUE_LIMIT + DEFAULT_HISTORY_LIMIT)
        .map(to_queue_event_item)
        .collect::<Vec<_>>();
    let queued_count = queue
        .in_flight
        .iter()
        .chain(queue.pending.iter())
        .filter(|item| {
            matches!(
                item.status,
                SpeechQueueItemStatus::Queued
                    | SpeechQueueItemStatus::Speaking
                    | SpeechQueueItemStatus::Error
            )
        })
        .count();
    let phase = if queue.paused {
        SpeechQueuePhase::Paused
    } else if queue.in_flight.is_some() {
        SpeechQueuePhase::Speaking
    } else if queue.pending.is_empty()
        && queue
            .history
            .iter()
            .any(|item| item.status == SpeechQueueItemStatus::Error)
    {
        SpeechQueuePhase::Error
    } else {
        SpeechQueuePhase::Idle
    };
    crate::app_events::SpeechQueueUpdatedEvent {
        revision: 0,
        queued_count,
        items,
        phase,
        warning,
        occurred_at_ms: chrono::Utc::now().timestamp_millis().max(0) as u64,
    }
}

#[cfg(any(feature = "app", test))]
fn to_queue_event_item(item: &SpeechQueueItem) -> SpeechQueueItemEvent {
    SpeechQueueItemEvent {
        id: item.id.clone(),
        source_message_id: item.source_message_id.clone(),
        user_display_name: item.user_display_name.clone(),
        text: item.text.clone(),
        status: item.status.clone(),
        outcome: item.outcome.clone(),
    }
}
