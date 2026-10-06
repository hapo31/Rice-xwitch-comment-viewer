use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum SpeechStatus {
    Idle,
    Speaking,
    Paused,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum SpeechAdapterHealth {
    Unknown,
    Connected,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
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
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum SpeechQueuePhase {
    Idle,
    Speaking,
    Paused,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpeechLogLevel {
    Info,
    Warning,
    Error,
}

/// Queue data prepared by the domain before an outer layer converts it to an event DTO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpeechQueueSnapshot {
    pub(crate) queued_count: usize,
    pub(crate) items: Vec<SpeechQueueItemSnapshot>,
    pub(crate) phase: SpeechQueuePhase,
    pub(crate) warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpeechQueueItemSnapshot {
    pub(crate) id: String,
    pub(crate) source_message_id: Option<String>,
    pub(crate) user_display_name: String,
    pub(crate) text: String,
    pub(crate) status: SpeechQueueItemStatus,
    pub(crate) outcome: Option<crate::speech::outcome::SpeechQueueOutcome>,
}
