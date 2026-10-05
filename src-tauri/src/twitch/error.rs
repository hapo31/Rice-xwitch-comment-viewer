//! Typed failure policy and presentation. Japanese wording never controls state.
#[derive(Debug, thiserror::Error)]
pub(super) enum TwitchApiError {
    #[error("Twitch API HTTP {status}: {message}")]
    Http {
        status: u16,
        code: Option<String>,
        message: String,
    },
    #[error(transparent)]
    Transport(#[from] reqwest::Error),
}

impl TwitchApiError {
    pub(super) fn is_transient(&self) -> bool {
        match self {
            Self::Http { status, .. } => *status >= 500,
            Self::Transport(error) => error.is_timeout(),
        }
    }

    pub(super) fn auth_failure(&self) -> Option<TwitchAuthFailure> {
        match self {
            Self::Http { status: 401, .. } => Some(TwitchAuthFailure::InvalidAccessToken),
            Self::Http {
                code: Some(code), ..
            } if code == "invalid_grant" => Some(TwitchAuthFailure::InvalidGrant),
            _ => None,
        }
    }

    pub(super) fn user_message(&self) -> String {
        match self {
            Self::Http { status: 401, .. } => {
                "Twitch の認証が無効です。Login から再ログインしてください。".to_string()
            }
            Self::Http {
                code: Some(code), ..
            } if code == "invalid_grant" => {
                "Twitch の認証期限が切れたか取り消されました。Login から再ログインしてください。"
                    .to_string()
            }
            Self::Http {
                status, message, ..
            } => {
                format!("Twitch API との通信に失敗しました（HTTP {status}）: {message}")
            }
            Self::Transport(error) => format!("Twitch API との通信に失敗しました: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TwitchAuthFailure {
    InvalidAccessToken,
    InvalidGrant,
}

#[cfg(feature = "app")]
#[derive(Debug, thiserror::Error)]
pub(super) enum SubscriptionRequestError {
    #[error("Twitch EventSub 購読の認証が拒否されました。")]
    Unauthorized,
    #[error("{0}")]
    AuthRequired(String),
    #[error(transparent)]
    Retryable(#[from] anyhow::Error),
    #[error(transparent)]
    Permanent(TwitchApiError),
}

#[cfg(feature = "app")]
#[derive(Debug, thiserror::Error)]
pub(super) enum EventSubTerminalError {
    #[error("{message}")]
    AuthRequired { message: String },
    #[error("{message}")]
    Permanent { message: String },
}

#[cfg(test)]
pub(super) fn is_definitive_auth_failure(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<TwitchApiError>()
        .and_then(TwitchApiError::auth_failure)
        .is_some()
}

pub(super) fn subscription_error_user_message(error: &TwitchApiError) -> String {
    match error {
        TwitchApiError::Http { status: 400, .. } => {
            "Twitch EventSub 購読の条件が無効です。接続チャンネルとアプリ設定を確認してください。"
                .to_string()
        }
        TwitchApiError::Http { status: 403, .. } => {
            "Twitch EventSub に必要な権限がありません。Login から再ログインしてください。"
                .to_string()
        }
        TwitchApiError::Http { status, .. } => format!(
            "Twitch EventSub 購読を開始できませんでした（HTTP {status}）。再接続せず停止しました。"
        ),
        TwitchApiError::Transport(error) => {
            format!("Twitch EventSub との通信に失敗しました: {error}")
        }
    }
}

pub(super) fn retryable_auth_error_message(error: &anyhow::Error) -> String {
    format!("Twitch の認証確認中に一時的な通信エラーが発生しました。ネットワークを確認して再試行してください: {error}")
}

pub(super) fn to_twitch_user_message(error: anyhow::Error) -> String {
    error
        .downcast_ref::<TwitchApiError>()
        .map(TwitchApiError::user_message)
        .unwrap_or_else(|| format!("Twitch 連携でエラーが発生しました: {error}"))
}

pub(super) fn to_secure_store_user_message(error: anyhow::Error) -> String {
    format!(
        "Twitch 認証情報を安全に削除できませんでした。OS の資格情報ストアを確認してから再度解除してください: {error}"
    )
}

#[cfg(feature = "app")]
pub(super) fn to_session_only_user_message(error: anyhow::Error) -> String {
    format!(
        "OS の資格情報ストアに保存できなかったため、今回の Twitch ログインはこの起動中だけ有効です。認証情報ファイルは作成していません。アプリを再起動したら再ログインしてください。OS の資格情報ストアを確認してください: {error}"
    )
}

#[cfg(feature = "app")]
pub(super) fn to_secure_store_load_user_message(error: anyhow::Error) -> String {
    format!(
        "OS の資格情報ストアから Twitch 認証情報を読み込めませんでした。資格情報ストアがロックまたは一時的に利用できない可能性があります。OS の資格情報ストアを確認してから再起動するか、Login から再ログインしてください: {error}"
    )
}

#[cfg(feature = "app")]
pub(super) fn to_legacy_auth_user_message(error: anyhow::Error) -> String {
    format!(
        "以前の平文 Twitch 認証情報（Linux: ~/.rice/twitch-auth.json）を OS の資格情報ストアへ移行できませんでした。安全のため読み込まず、再ログインが必要です。ファイルを削除し、Twitch の「設定と接続」からこのアプリのアクセスを取り消してから再ログインしてください: {error}"
    )
}

#[cfg(feature = "app")]
pub(super) fn to_auth_recovery_failure_user_message(
    secure_load_error: Option<anyhow::Error>,
    legacy_error: anyhow::Error,
) -> String {
    match secure_load_error {
        Some(secure_error) => format!(
            "{} さらに、{}",
            to_secure_store_load_user_message(secure_error),
            to_legacy_auth_user_message(legacy_error)
        ),
        None => to_legacy_auth_user_message(legacy_error),
    }
}

#[cfg(feature = "app")]
pub(super) fn to_legacy_cleanup_user_message(error: anyhow::Error) -> String {
    format!(
        "以前の平文 Twitch 認証情報（Linux: ~/.rice/twitch-auth.json）が残っています。ファイルを削除し、Twitch の「設定と接続」からこのアプリのアクセスを取り消して再ログインしてください: {error}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_message_does_not_turn_a_condition_error_into_an_auth_failure() {
        let error = TwitchApiError::Http {
            status: 400,
            code: None,
            message: "認証期限が切れたので再ログインしてください".into(),
        };
        assert_eq!(error.auth_failure(), None);
        assert!(!error.is_transient());
        assert!(!is_definitive_auth_failure(&anyhow::Error::new(error)));
    }

    #[test]
    fn status_and_oauth_code_control_policy_even_when_display_wording_disagrees() {
        let unauthorized = TwitchApiError::Http {
            status: 401,
            code: None,
            message: "通信成功".into(),
        };
        assert_eq!(
            unauthorized.auth_failure(),
            Some(TwitchAuthFailure::InvalidAccessToken)
        );
        let revoked = TwitchApiError::Http {
            status: 400,
            code: Some("invalid_grant".into()),
            message: "一時的なエラー".into(),
        };
        assert_eq!(
            revoked.auth_failure(),
            Some(TwitchAuthFailure::InvalidGrant)
        );
        assert!(!revoked.is_transient());
        let unavailable = TwitchApiError::Http {
            status: 503,
            code: None,
            message: "再接続せず停止してください".into(),
        };
        assert!(unavailable.is_transient());
        assert_eq!(unavailable.auth_failure(), None);
    }
}
