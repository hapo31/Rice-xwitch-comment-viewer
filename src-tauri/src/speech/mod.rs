pub mod bouyomi;
#[cfg(feature = "app")]
pub mod commands;
pub(crate) mod destination;
pub mod endpoint;
#[cfg(feature = "app")]
mod events;
mod factory;
mod failure;
mod formatter;
pub mod outcome;
mod queue;
#[cfg(feature = "app")]
mod queue_commands;
pub mod runtime;
mod types;
#[cfg(any(feature = "app", test))]
mod worker;

pub use failure::{FailureCode, SpeechFailure};
#[cfg(test)]
pub(crate) use formatter::{SpeechFormatDecision, SpeechFormatter, SpeechFormatterOptions};
#[cfg(test)]
pub(crate) use outcome::BlockedReason;
#[cfg(any(feature = "app", test))]
pub use queue::SpeechQueueState;
use serde::{Deserialize, Serialize};
pub use types::{SpeechAdapterHealth, SpeechQueueItemStatus, SpeechQueuePhase, SpeechStatus};
#[cfg(any(feature = "app", test))]
pub(crate) use types::{SpeechLogLevel, SpeechQueueItemSnapshot, SpeechQueueSnapshot};
pub type SpeechFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeechRequest {
    pub id: String,
    pub source_message_id: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum SpeechHealth {
    Connected,
    Disconnected { failure: SpeechFailure },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeechResult {
    Accepted,
}

/// Object-safe protocol boundary. Production callers use SpeechRuntime's locked
/// session; methods send without taking the common dispatch gate a second time.
pub trait SpeechAdapter: Send + Sync {
    fn health_check(&self) -> SpeechFuture<'_, Result<SpeechHealth, SpeechFailure>>;
    fn speak(
        &self,
        request: SpeechRequest,
    ) -> SpeechFuture<'_, Result<SpeechResult, SpeechFailure>>;
    fn pause(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>>;
    fn resume(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>>;
    fn skip(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>>;
    fn clear(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>>;
    /// Called after releasing the submission gate, so controls remain possible.
    /// Adapters serialize their individual polling requests through that gate.
    fn wait_for_completion(&self) -> SpeechFuture<'_, SpeechPlaybackCompletion>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechControl {
    Pause,
    Resume,
    Skip,
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeechPlaybackCompletion {
    Completed,
    Unconfirmed(SpeechFailure),
}

#[cfg(test)]
use crate::twitch::{ChatMessage, MessageFragment};
#[cfg(test)]
use chrono::{DateTime, Utc};
#[cfg(test)]
use formatter::DEFAULT_MAX_COMMENT_LENGTH;
#[cfg(test)]
pub(crate) use queue::DEFAULT_HISTORY_LIMIT;
#[cfg(test)]
pub(crate) use queue::{
    enqueue_message, QueueEnqueueOutcome, SpeechQueueDeliveryState, SpeechQueueFailureTransition,
    SpeechQueueItem, DEFAULT_QUEUE_LIMIT,
};
#[cfg(test)]
use queue::{
    suppress_repeated_message, RepeatSuppressionScope, MAX_REPEAT_SUPPRESSION_ENTRIES,
    MAX_REPEAT_SUPPRESSION_WINDOW, REPEAT_SUPPRESSION_CLEANUP_BATCH,
};
#[cfg(feature = "app")]
pub(crate) use queue_commands::{
    begin_queue_control, cancel_queue_control, clear_speech_queue, emit_current_queue,
    enqueue_chat_message_for_speech, pause_queue, resume_queue, skip_current_queue_item,
    speech_queue_dismiss, speech_queue_dismiss_history, speech_queue_reload, speech_queue_remove,
    speech_queue_retry,
};
#[cfg(test)]
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;
