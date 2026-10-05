use crate::app_events::SpeechStatus;
use std::io::{Error, ErrorKind};

#[derive(Debug, thiserror::Error)]
pub(crate) enum BouyomiError {
    #[error("Bouyomi destination policy: {0}")]
    Destination(crate::settings::validation::ValidationError),
    #[error("invalid Bouyomi configuration: {0}")]
    Configuration(String),
    #[error("Bouyomi connect timed out")]
    ConnectTimeout,
    #[error("Bouyomi connect failed: {0}")]
    ConnectIo(#[source] Error),
    #[error("Bouyomi write timed out; delivery is unknown")]
    WriteTimeout,
    #[error("Bouyomi write failed; delivery is unknown: {0}")]
    WriteIo(#[source] Error),
    #[error("Bouyomi response timed out")]
    ResponseTimeout,
    #[error("Bouyomi response failed: {0}")]
    ResponseIo(#[source] Error),
    #[error("invalid Bouyomi boolean response: {0}")]
    InvalidResponse(u8),
}

pub(crate) use crate::speech::{FailureCode, SpeechFailure};

pub(crate) fn classify_error(error: anyhow::Error) -> SpeechFailure {
    if let Some(BouyomiError::Destination(failure)) = error.downcast_ref::<BouyomiError>() {
        return SpeechFailure {
            code: FailureCode::Configuration,
            status: SpeechStatus::Error,
            retryable: false,
            user_message: format!("{} {}", failure.message, failure.recovery),
            detail: format!("{}: {}", failure.field, failure.code),
        };
    }
    if let Some(failure) = error.downcast_ref::<SpeechFailure>() {
        let mut classified = failure.clone();
        classified.detail = format!("{}; {}", classified.detail, error);
        return classified;
    }
    use FailureCode as Code;
    use SpeechStatus::{Disconnected, Error as Failed};

    let (code, status, retryable) = match error.downcast_ref::<BouyomiError>() {
        Some(BouyomiError::Configuration(_) | BouyomiError::Destination(_)) => {
            (Code::Configuration, Failed, false)
        }
        Some(BouyomiError::ConnectTimeout) => (Code::ConnectTimeout, Disconnected, true),
        Some(BouyomiError::ConnectIo(source)) => match source.kind() {
            ErrorKind::ConnectionRefused => (Code::ConnectionRefused, Disconnected, true),
            ErrorKind::TimedOut => (Code::ConnectTimeout, Disconnected, true),
            ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::NotConnected
            | ErrorKind::BrokenPipe => (Code::ConnectionLost, Disconnected, true),
            ErrorKind::InvalidInput | ErrorKind::AddrNotAvailable => {
                (Code::Configuration, Failed, false)
            }
            ErrorKind::PermissionDenied => (Code::PermissionDenied, Failed, false),
            ErrorKind::NetworkUnreachable | ErrorKind::HostUnreachable | ErrorKind::NotFound => {
                (Code::ConnectFailed, Disconnected, true)
            }
            _ => (Code::ConnectFailed, Disconnected, false),
        },
        Some(BouyomiError::WriteTimeout) => (Code::WriteTimeout, Disconnected, false),
        Some(BouyomiError::WriteIo(source)) => match source.kind() {
            ErrorKind::TimedOut => (Code::WriteTimeout, Disconnected, false),
            ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::NotConnected
            | ErrorKind::BrokenPipe => (Code::ConnectionLost, Disconnected, false),
            ErrorKind::PermissionDenied => (Code::PermissionDenied, Failed, false),
            ErrorKind::InvalidInput => (Code::Configuration, Failed, false),
            _ => (Code::WriteFailed, Failed, false),
        },
        Some(BouyomiError::ResponseTimeout) => (Code::ResponseTimeout, Failed, false),
        Some(BouyomiError::ResponseIo(source)) => match source.kind() {
            ErrorKind::TimedOut => (Code::ResponseTimeout, Failed, false),
            ErrorKind::UnexpectedEof => (Code::ProtocolMismatch, Failed, false),
            ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::NotConnected
            | ErrorKind::BrokenPipe => (Code::ConnectionLost, Disconnected, false),
            _ => (Code::ResponseFailed, Failed, false),
        },
        Some(BouyomiError::InvalidResponse(_)) => (Code::ProtocolMismatch, Failed, false),
        None => (Code::Unknown, Failed, false),
    };
    let cause = match code {
        Code::Configuration => "棒読みちゃんの接続設定が無効です。ホストとポートを修正してください。",
        Code::ConnectionRefused => "棒読みちゃんに接続できません。起動中でアプリ連携/TCP受付が有効か確認してください。",
        Code::ConnectTimeout => "棒読みちゃんへの接続がタイムアウトしました。接続先と通信設定を確認してください。",
        Code::ConnectFailed => "棒読みちゃんへの接続に失敗しました。ホスト名とネットワークを確認してください。",
        Code::ConnectionLost => "棒読みちゃんとの接続が切断されました。相手の起動状態と通信設定を確認してください。",
        Code::PermissionDenied => "棒読みちゃんとの通信が許可されていません。セキュリティソフトの通信設定を確認してください。",
        Code::WriteTimeout => "棒読みちゃんへの送信がタイムアウトしました。届いた可能性があるため、自動再送しません。",
        Code::WriteFailed => "棒読みちゃんへの送信に失敗しました。届いた可能性があるため、自動再送しません。",
        Code::ResponseTimeout => "棒読みちゃんの状態応答がタイムアウトしました。ポート競合やTCP受付を確認してください。",
        Code::ResponseFailed => "棒読みちゃんの状態応答を受信できません。相手側の状態を確認してください。",
        Code::ProtocolMismatch => "棒読みちゃんと互換性のない状態応答です。相手側の切断や別アプリとのポート競合を確認してください。",
        Code::Unknown => "棒読みちゃん連携で予期しないエラーが発生しました。Logsの詳細を確認してください。",
    };
    SpeechFailure {
        code,
        status,
        retryable,
        user_message: format!("{cause} ［診断］を実行してください。"),
        detail: format!("{error:#}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_mapping_is_independent_of_locale_and_os_error_numbers() {
        for (kind, expected, status, retryable) in [
            (
                ErrorKind::ConnectionRefused,
                FailureCode::ConnectionRefused,
                SpeechStatus::Disconnected,
                true,
            ),
            (
                ErrorKind::TimedOut,
                FailureCode::ConnectTimeout,
                SpeechStatus::Disconnected,
                true,
            ),
            (
                ErrorKind::ConnectionReset,
                FailureCode::ConnectionLost,
                SpeechStatus::Disconnected,
                true,
            ),
            (
                ErrorKind::ConnectionAborted,
                FailureCode::ConnectionLost,
                SpeechStatus::Disconnected,
                true,
            ),
            (
                ErrorKind::NotConnected,
                FailureCode::ConnectionLost,
                SpeechStatus::Disconnected,
                true,
            ),
            (
                ErrorKind::BrokenPipe,
                FailureCode::ConnectionLost,
                SpeechStatus::Disconnected,
                true,
            ),
            (
                ErrorKind::PermissionDenied,
                FailureCode::PermissionDenied,
                SpeechStatus::Error,
                false,
            ),
            (
                ErrorKind::InvalidInput,
                FailureCode::Configuration,
                SpeechStatus::Error,
                false,
            ),
            (
                ErrorKind::AddrNotAvailable,
                FailureCode::Configuration,
                SpeechStatus::Error,
                false,
            ),
            (
                ErrorKind::NotFound,
                FailureCode::ConnectFailed,
                SpeechStatus::Disconnected,
                true,
            ),
            (
                ErrorKind::NetworkUnreachable,
                FailureCode::ConnectFailed,
                SpeechStatus::Disconnected,
                true,
            ),
            (
                ErrorKind::Other,
                FailureCode::ConnectFailed,
                SpeechStatus::Disconnected,
                false,
            ),
        ] {
            for message in [
                "説明のみ",
                "Verbindung abgelehnt",
                "Connection refused (os error 10061)",
            ] {
                let failure =
                    classify_error(BouyomiError::ConnectIo(Error::new(kind, message)).into());
                assert_eq!(
                    (failure.code, failure.status, failure.retryable),
                    (expected, status, retryable)
                );
                assert!(!failure.user_message.contains(message));
                assert!(failure.detail.contains(message));
            }
        }
    }

    #[test]
    fn phase_is_preserved_and_delivery_uncertainty_disables_retry() {
        for (error, code, status) in [
            (
                BouyomiError::ConnectTimeout,
                FailureCode::ConnectTimeout,
                SpeechStatus::Disconnected,
            ),
            (
                BouyomiError::WriteTimeout,
                FailureCode::WriteTimeout,
                SpeechStatus::Disconnected,
            ),
            (
                BouyomiError::ResponseTimeout,
                FailureCode::ResponseTimeout,
                SpeechStatus::Error,
            ),
            (
                BouyomiError::InvalidResponse(2),
                FailureCode::ProtocolMismatch,
                SpeechStatus::Error,
            ),
            (
                BouyomiError::WriteIo(Error::from(ErrorKind::BrokenPipe)),
                FailureCode::ConnectionLost,
                SpeechStatus::Disconnected,
            ),
            (
                BouyomiError::WriteIo(Error::from(ErrorKind::Other)),
                FailureCode::WriteFailed,
                SpeechStatus::Error,
            ),
            (
                BouyomiError::ResponseIo(Error::from(ErrorKind::UnexpectedEof)),
                FailureCode::ProtocolMismatch,
                SpeechStatus::Error,
            ),
            (
                BouyomiError::ResponseIo(Error::from(ErrorKind::Other)),
                FailureCode::ResponseFailed,
                SpeechStatus::Error,
            ),
        ] {
            let failure = classify_error(anyhow::Error::from(error).context("outer context"));
            assert_eq!((failure.code, failure.status), (code, status));
            assert_eq!(failure.retryable, code == FailureCode::ConnectTimeout);
            assert!(failure.detail.contains("outer context"));
        }
    }

    #[test]
    fn unknown_text_is_not_classified_as_transport_and_is_kept_only_in_logs() {
        let failure = classify_error(anyhow::anyhow!(
            "Connection refused os error 111 operation timed out"
        ));
        assert_eq!(failure.code, FailureCode::Unknown);
        assert_eq!(failure.status, SpeechStatus::Error);
        assert!(!failure.retryable);
        assert!(!failure.user_message.contains("os error"));
        assert!(failure.log_message().contains("os error 111"));
    }

    #[test]
    fn typed_failure_survives_trait_error_and_context_boundaries() {
        let original = classify_error(BouyomiError::WriteTimeout.into());
        let classified =
            classify_error(anyhow::Error::from(original.clone()).context("adapter boundary"));
        assert_eq!(classified.code, original.code);
        assert_eq!(classified.status, original.status);
        assert_eq!(classified.retryable, original.retryable);
        assert_eq!(classified.user_message, original.user_message);
        assert!(classified.detail.contains("adapter boundary"));
    }

    #[test]
    fn native_os_errors_are_normalized_by_std_not_by_message_search() {
        #[cfg(windows)]
        let cases = [
            (10061, ErrorKind::ConnectionRefused),
            (10060, ErrorKind::TimedOut),
            (10054, ErrorKind::ConnectionReset),
            (10053, ErrorKind::ConnectionAborted),
        ];
        #[cfg(target_os = "linux")]
        let cases = [
            (libc::ECONNREFUSED, ErrorKind::ConnectionRefused),
            (libc::ETIMEDOUT, ErrorKind::TimedOut),
            (libc::ECONNRESET, ErrorKind::ConnectionReset),
            (libc::ECONNABORTED, ErrorKind::ConnectionAborted),
        ];
        #[cfg(not(any(windows, target_os = "linux")))]
        let cases: [(i32, ErrorKind); 0] = [];
        for (native, kind) in cases {
            let source = Error::from_raw_os_error(native);
            assert_eq!(source.kind(), kind);
            let native_failure = classify_error(BouyomiError::ConnectIo(source).into());
            let synthetic_failure =
                classify_error(BouyomiError::ConnectIo(Error::new(kind, "別の表示文")).into());
            assert_eq!(native_failure.status, SpeechStatus::Disconnected);
            assert_eq!(native_failure.code, synthetic_failure.code);
            assert_eq!(native_failure.user_message, synthetic_failure.user_message);
        }
    }
}
