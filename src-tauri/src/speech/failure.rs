use super::{SpeechAdapterHealth, SpeechStatus};

/// Adapter-independent delivery/health classification. Adapters map their native
/// errors here; the scheduler never inspects protocol errors or display strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
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

impl FailureCode {
    pub(crate) fn safe_message(self) -> &'static str {
        match self {
            Self::Configuration => "読み上げ先の接続設定が無効です。",
            Self::ConnectionRefused => {
                "読み上げ先に接続できません。起動とTCP受付を確認してください。"
            }
            Self::ConnectTimeout => "読み上げ先への接続がタイムアウトしました。",
            Self::ConnectFailed => "読み上げ先への接続に失敗しました。",
            Self::ConnectionLost => "読み上げ先との接続が切断されました。",
            Self::PermissionDenied => "読み上げ先との通信が許可されていません。",
            Self::WriteTimeout => "読み上げ先への送信がタイムアウトしました。",
            Self::WriteFailed => "読み上げ先への送信に失敗しました。",
            Self::ResponseTimeout => "読み上げ先の状態応答がタイムアウトしました。",
            Self::ResponseFailed => "読み上げ先の状態応答を受信できません。",
            Self::ProtocolMismatch => "読み上げ先と互換性のない状態応答です。",
            Self::Unknown => "読み上げ連携で予期しないエラーが発生しました。",
        }
    }
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
