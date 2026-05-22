use crate::twitch::ChatMessage;
use serde::Serialize;
#[cfg(feature = "app")]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(feature = "app")]
use tauri::{AppHandle, Emitter, Runtime};

pub const TWITCH_STATUS_EVENT: &str = "twitch://status";
#[allow(dead_code)]
pub const TWITCH_CHAT_MESSAGE_EVENT: &str = "twitch://chat-message";
pub const SPEECH_QUEUE_UPDATED_EVENT: &str = "speech://queue-updated";
pub const SPEECH_STATUS_EVENT: &str = "speech://status";
pub const APP_LOG_EVENT: &str = "app://log";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLogEvent {
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
    pub status: TwitchStatus,
    pub message: Option<String>,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TwitchStatus {
    Disconnected,
    Connecting,
    Connected,
    #[allow(dead_code)]
    Reconnecting,
    AuthRequired,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechStatusEvent {
    pub status: SpeechStatus,
    pub message: Option<String>,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeechStatus {
    Idle,
    Speaking,
    Paused,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechQueueUpdatedEvent {
    pub queued_count: usize,
    pub warning: Option<String>,
    pub occurred_at_ms: u64,
}

#[cfg(feature = "app")]
pub fn emit_app_log<R: Runtime>(
    app: &AppHandle<R>,
    level: AppLogLevel,
    message: impl Into<String>,
) {
    let payload = AppLogEvent {
        level,
        message: message.into(),
        occurred_at_ms: current_timestamp_ms(),
    };
    let _ = app.emit(APP_LOG_EVENT, payload);
}

#[cfg(feature = "app")]
pub fn emit_twitch_status<R: Runtime>(
    app: &AppHandle<R>,
    status: TwitchStatus,
    message: Option<String>,
) {
    let payload = TwitchStatusEvent {
        status,
        message,
        occurred_at_ms: current_timestamp_ms(),
    };
    let _ = app.emit(TWITCH_STATUS_EVENT, payload);
}

#[cfg(feature = "app")]
pub fn emit_twitch_chat_message<R: Runtime>(app: &AppHandle<R>, message: ChatMessage) {
    let _ = app.emit(TWITCH_CHAT_MESSAGE_EVENT, message);
}

#[cfg(feature = "app")]
pub fn emit_speech_status<R: Runtime>(
    app: &AppHandle<R>,
    status: SpeechStatus,
    message: Option<String>,
) {
    let payload = SpeechStatusEvent {
        status,
        message,
        occurred_at_ms: current_timestamp_ms(),
    };
    let _ = app.emit(SPEECH_STATUS_EVENT, payload);
}

#[cfg(feature = "app")]
pub fn emit_speech_queue_updated<R: Runtime>(
    app: &AppHandle<R>,
    queued_count: usize,
    warning: Option<String>,
) {
    let payload = SpeechQueueUpdatedEvent {
        queued_count,
        warning,
        occurred_at_ms: current_timestamp_ms(),
    };
    let _ = app.emit(SPEECH_QUEUE_UPDATED_EVENT, payload);
}

#[cfg(feature = "app")]
fn current_timestamp_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as u64,
        Err(_) => 0,
    }
}
