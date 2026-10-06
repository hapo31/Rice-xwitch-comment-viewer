//! Bounded, safe item outcomes. Never store adapter details, configuration,
//! matched NG words, tokens or display-name interpolation here.
use super::{FailureCode, SpeechFailure};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum BlockedReason {
    RepeatSuppressed,
    BlockedUser,
    BlockedWord,
    BlockedUrl,
    EmptyAfterFormatting,
}

impl BlockedReason {
    pub fn message(self) -> &'static str {
        match self {
            Self::RepeatSuppressed => "同じユーザーの連投を抑制しました。",
            Self::BlockedUser => "NG ユーザーに一致しました。",
            Self::BlockedWord => "NG ワードを含むため読み上げません。",
            Self::BlockedUrl => "URL を含むため読み上げません。",
            Self::EmptyAfterFormatting => "読み上げる本文がありません。",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum SkippedReason {
    Overflow,
    UserSkip,
    Removed,
    Cleared,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum RecoveryAction {
    ReviewFilters,
    ReviewQueue,
    DiagnoseSpeech,
    ConfirmDelivery,
    None,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct OutcomeDetails {
    pub message: String,
    /// Failure category permits a new safe attempt, not an unused retry budget.
    /// Terminal error history is never automatically resent.
    pub retryable: bool,
    pub recovery_action: RecoveryAction,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum SpeechQueueOutcome {
    Blocked {
        reason_code: BlockedReason,
        #[serde(flatten)]
        details: OutcomeDetails,
    },
    Skipped {
        reason_code: SkippedReason,
        #[serde(flatten)]
        details: OutcomeDetails,
    },
    Error {
        reason_code: FailureCode,
        #[serde(flatten)]
        details: OutcomeDetails,
    },
}

pub(super) fn now_ms() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}

impl SpeechQueueOutcome {
    pub(super) fn blocked(reason_code: BlockedReason, occurred_at_ms: u64) -> Self {
        Self::Blocked {
            reason_code,
            details: OutcomeDetails {
                message: reason_code.message().into(),
                retryable: false,
                recovery_action: RecoveryAction::ReviewFilters,
                occurred_at_ms,
            },
        }
    }

    pub(super) fn skipped(reason_code: SkippedReason, occurred_at_ms: u64) -> Self {
        let (message, recovery_action) = match reason_code {
            SkippedReason::Overflow => (
                "キュー上限に達したため、古い未読チャットを落としました。",
                RecoveryAction::ReviewQueue,
            ),
            SkippedReason::UserSkip => ("利用者がスキップしました。", RecoveryAction::None),
            SkippedReason::Removed => ("利用者が個別に削除しました。", RecoveryAction::None),
            SkippedReason::Cleared => ("利用者がキューをクリアしました。", RecoveryAction::None),
        };
        Self::Skipped {
            reason_code,
            details: OutcomeDetails {
                message: message.into(),
                retryable: false,
                recovery_action,
                occurred_at_ms,
            },
        }
    }

    pub(super) fn error(failure: &SpeechFailure, accepted: bool, occurred_at_ms: u64) -> Self {
        let uncertain = accepted
            || (!failure.retryable
                && matches!(
                    failure.code,
                    FailureCode::WriteTimeout
                        | FailureCode::WriteFailed
                        | FailureCode::ConnectionLost
                        | FailureCode::Unknown
                ));
        let mut message = failure.code.safe_message().to_string();
        if uncertain {
            message.push_str(" 届いた可能性があるため、自動再送していません。");
        }
        Self::Error {
            reason_code: failure.code,
            details: OutcomeDetails {
                message,
                retryable: failure.retryable && !accepted,
                recovery_action: if uncertain {
                    RecoveryAction::ConfirmDelivery
                } else {
                    RecoveryAction::DiagnoseSpeech
                },
                occurred_at_ms,
            },
        }
    }
}

#[cfg(test)]
mod tests;
