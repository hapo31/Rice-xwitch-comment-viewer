use super::queue::SpeechQueueState;
#[cfg(feature = "app")]
use super::worker;
use super::SpeechLogLevel;
#[cfg(feature = "app")]
use crate::app_events::SpeechStatus;
#[cfg(feature = "app")]
use crate::app_events::{
    emit_app_log, emit_speech_adapter_health, emit_speech_queue_updated, emit_speech_status,
    AppLogLevel,
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
    fn log(&self, level: SpeechLogLevel, message: String) {
        let level = match level {
            SpeechLogLevel::Warning => AppLogLevel::Warning,
            SpeechLogLevel::Error => AppLogLevel::Error,
        };
        emit_app_log(&self.0, level, message);
    }
}

#[cfg(feature = "app")]
pub(super) fn emit_queue_snapshot(
    app: &tauri::AppHandle<tauri::Wry>,
    queue: &SpeechQueueState,
    warning: Option<String>,
) {
    let payload = crate::app_events::speech_queue_updated_event(queue.snapshot(warning));
    emit_speech_queue_updated(app, payload);
}
