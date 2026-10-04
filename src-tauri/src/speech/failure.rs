use crate::app_events::{SpeechAdapterHealth, SpeechStatus};

/// Adapter-independent delivery/health classification. Adapters map their native
/// errors here; the scheduler never inspects protocol errors or display strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCode {
    Configuration,
    ConnectionRefused,
    ConnectTimeout,
    ConnectFailed,
    ConnectionLost,
    PermissionDenied,
    WriteTimeout,
    WriteFailed,
    ResponseTimeout,
    ResponseFailed,
    ProtocolMismatch,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{user_message}")]
pub struct SpeechFailure {
    pub code: FailureCode,
    pub status: SpeechStatus,
    /// True only when a transient failure occurred before any delivery.
    pub retryable: bool,
    pub user_message: String,
    pub detail: String,
}

impl SpeechFailure {
    pub fn adapter_health(&self) -> SpeechAdapterHealth {
        if self.status == SpeechStatus::Disconnected {
            SpeechAdapterHealth::Disconnected
        } else {
            SpeechAdapterHealth::Error
        }
    }

    pub fn unknown(detail: String) -> Self {
        Self {
            code: FailureCode::Unknown,
            status: SpeechStatus::Error,
            retryable: false,
            user_message:
                "読み上げ連携で予期しないエラーが発生しました。Logsの詳細を確認してください。"
                    .to_string(),
            detail,
        }
    }

    pub fn log_message(&self) -> String {
        format!(
            "{} [code={:?}, retryable={}]: {}",
            self.user_message, self.code, self.retryable, self.detail
        )
    }
}
