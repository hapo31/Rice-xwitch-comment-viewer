//! Device authorization, validation and generation-safe credential lifecycle.
use super::auth_state::{
    ensure_required_twitch_scopes, token_scopes, EventSubAuthCredentials, PendingDeviceAuth,
    TwitchAuthPollResult, TwitchAuthState, TwitchAuthValidationResult, TwitchDeviceAuthStart,
    TwitchToken, TwitchUserProfile,
};
use super::auth_store::{AuthClearOutcome, AuthSaveOutcome, TwitchAuthStore};
use super::error::{
    retryable_auth_error_message, to_secure_store_user_message, to_twitch_user_message,
};
use super::oauth::{refresh_and_validate, DeviceOAuthTransport, PollAuthError};
use crate::app_events::{AppLogLevel, TwitchAuthRequiredReason, TwitchStatus, TwitchStatusDomain};

/// Dependencies owned by the application adapter or a deterministic test runtime.
/// Authentication never holds the state lock across transport or credential I/O.
pub(super) trait AuthRuntime: DeviceOAuthTransport + Sync {
    fn auth(&self) -> &std::sync::Arc<std::sync::Mutex<TwitchAuthState>>;
    fn store(&self) -> &TwitchAuthStore;
    fn client_id(&self) -> String;
    fn now(&self) -> std::time::SystemTime;
    fn cancel_chat(&self) -> Result<bool, String>;
    fn auth_status(
        &self,
        domain: TwitchStatusDomain,
        status: TwitchStatus,
        message: Option<String>,
    );
    fn auth_log(&self, level: AppLogLevel, message: impl Into<String>);
    fn require_auth(&self, reason: TwitchAuthRequiredReason, message: impl Into<String>);
}

pub(super) struct TwitchAuthService<'a, R> {
    runtime: &'a R,
}

#[derive(Debug)]
pub(super) struct StaleCredentialResponse;
impl std::fmt::Display for StaleCredentialResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("古い Twitch 認証応答です")
    }
}
impl std::error::Error for StaleCredentialResponse {}

#[derive(Debug)]
pub(super) struct MissingTwitchScope(pub(super) String);
impl std::fmt::Display for MissingTwitchScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for MissingTwitchScope {}

/// Shared refresh/validate/rotate/save path used by Login validation and
/// EventSub's 401 recovery. Credential identity is checked across the network
/// request and again by the durable store before commit.
pub(super) async fn refresh_credentials_if_current(
    state: &impl AuthRuntime,
    credentials: &EventSubAuthCredentials,
) -> anyhow::Result<(TwitchUserProfile, Option<String>)> {
    let (token, profile) = match refresh_and_validate(state, credentials).await {
        Ok(result) => result,
        Err(error) => {
            if !credentials_are_current(
                state,
                credentials.generation,
                credentials.credential_revision,
                &credentials.access_token,
                &credentials.refresh_token,
            )
            .map_err(anyhow::Error::msg)?
            {
                return Err(anyhow::Error::new(StaleCredentialResponse));
            }
            return Err(error);
        }
    };
    if let Err(error) = ensure_required_twitch_scopes(&profile.scopes) {
        if !credentials_are_current(
            state,
            credentials.generation,
            credentials.credential_revision,
            &credentials.access_token,
            &credentials.refresh_token,
        )
        .map_err(anyhow::Error::msg)?
        {
            return Err(anyhow::Error::new(StaleCredentialResponse));
        }
        return Err(anyhow::Error::new(MissingTwitchScope(error.to_string())));
    }

    let (_, _, warning) = persist_credential_rotation(
        state.auth().clone(),
        state.store(),
        credentials,
        token,
        profile.clone(),
    )
    .await?;
    Ok((profile, warning))
}

pub(super) async fn persist_credential_rotation(
    auth_state: std::sync::Arc<std::sync::Mutex<TwitchAuthState>>,
    store: &TwitchAuthStore,
    credentials: &EventSubAuthCredentials,
    refreshed: super::auth_state::TokenResponse,
    profile: TwitchUserProfile,
) -> anyhow::Result<(String, bool, Option<String>)> {
    let (access_token, snapshot, generation, credential_revision) = {
        let mut auth = auth_state
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        auth.eventsub_credentials()?;
        if auth.generation != credentials.generation
            || !auth.credentials_match(
                credentials.credential_revision,
                &credentials.access_token,
                &credentials.refresh_token,
            )
        {
            return Err(anyhow::Error::new(StaleCredentialResponse));
        }
        let access = auth.replace_token(refreshed, profile)?;
        (
            access,
            auth.clone(),
            auth.generation,
            auth.credential_revision,
        )
    };
    let warning = match store
        .save_if_current(auth_state.clone(), generation, snapshot)
        .await?
    {
        AuthSaveOutcome::Saved(warning) => warning,
        AuthSaveOutcome::Stale => return Err(anyhow::Error::new(StaleCredentialResponse)),
    };
    let auth = auth_state
        .lock()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    if auth.generation != generation || auth.credential_revision != credential_revision {
        return Err(anyhow::Error::new(StaleCredentialResponse));
    }
    Ok((access_token, true, warning))
}

impl<'a, R: AuthRuntime> TwitchAuthService<'a, R> {
    pub(super) fn new(runtime: &'a R) -> Self {
        Self { runtime }
    }
    pub(super) async fn start(&self) -> Result<TwitchDeviceAuthStart, String> {
        let state = self.runtime;
        let client_id = state.client_id();

        if client_id.is_empty() {
            return Err("Twitch Client ID がビルド設定にありません。RICE_TWITCH_CLIENT_ID を設定してビルドしてください。".to_string());
        }

        let generation = {
            let mut auth = state.auth().lock().map_err(|error| error.to_string())?;
            let generation = auth.invalidate_operations();
            auth.credential_revision = auth.credential_revision.wrapping_add(1);
            generation
        };

        let response = state
            .device_code(&client_id)
            .await
            .map_err(to_twitch_user_message)?;
        let auth_start = TwitchDeviceAuthStart {
            user_code: response.user_code.clone(),
            verification_uri: response.verification_uri,
            expires_in: response.expires_in,
            expires_at_ms: state
                .now()
                .checked_add(std::time::Duration::from_secs(response.expires_in))
                .and_then(|deadline| deadline.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|deadline| deadline.as_millis() as u64)
                .unwrap_or(u64::MAX),
            interval: response.interval,
        };

        let mut auth = state.auth().lock().map_err(|error| error.to_string())?;
        if auth.generation != generation {
            return Err(
                "新しい Twitch 認証操作が開始されたため、古い認証コードを破棄しました。"
                    .to_string(),
            );
        }
        auth.token = None;
        auth.profile = None;
        auth.pending = Some(PendingDeviceAuth {
            generation,
            poll_in_flight: false,
            client_id,
            device_code: response.device_code,
            interval: response.interval,
        });
        state.auth_status(
            TwitchStatusDomain::Auth,
            TwitchStatus::AuthRequired,
            Some("Twitch 認証コードを発行しました。".to_string()),
        );
        state.auth_log(AppLogLevel::Info, "Twitch 認証コードを発行しました。");

        Ok(auth_start)
    }
    pub(super) async fn poll(&self) -> Result<TwitchAuthPollResult, String> {
        let state = self.runtime;
        let pending = {
            let mut auth = state.auth().lock().map_err(|error| error.to_string())?;
            let pending = auth
                .pending
                .as_mut()
                .ok_or_else(|| "Twitch 認証が開始されていません。".to_string())?;
            if pending.poll_in_flight {
                return Err("Twitch 認証を確認中です。完了までお待ちください。".to_string());
            }
            pending.poll_in_flight = true;
            pending.clone()
        };

        match state.poll_token(&pending).await {
            Ok(token) => {
                ensure_pending_auth_is_current(state, pending.generation)?;
                let profile = match state.validate(&token.access_token).await {
                    Ok(profile) => profile,
                    Err(error) => {
                        clear_poll_in_flight_if_current(state, pending.generation)?;
                        return Err(to_twitch_user_message(error));
                    }
                };
                let profile = TwitchUserProfile::from(profile);
                if let Err(error) = ensure_required_twitch_scopes(&profile.scopes) {
                    let message = error.to_string();
                    {
                        let mut auth = state.auth().lock().map_err(|error| error.to_string())?;
                        if auth.generation != pending.generation {
                            return Err(
                            "新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。"
                                .to_string(),
                        );
                        }
                        auth.pending = None;
                        auth.token = None;
                        auth.profile = None;
                    }
                    state.require_auth(
                        TwitchAuthRequiredReason::MissingRequiredScope,
                        message.clone(),
                    );
                    state.auth_log(AppLogLevel::Warning, message.clone());
                    return Err(message);
                }

                let auth_snapshot = {
                    let mut auth = state.auth().lock().map_err(|error| error.to_string())?;
                    if auth.generation != pending.generation {
                        return Err(
                            "新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。"
                                .to_string(),
                        );
                    }
                    auth.pending = None;
                    auth.profile = Some(profile.clone());
                    auth.credential_revision = auth.credential_revision.wrapping_add(1);
                    auth.token = Some(TwitchToken {
                        access_token: token.access_token,
                        refresh_token: token.refresh_token,
                        scopes: token_scopes(token.scope, &profile),
                        expires_in: token.expires_in,
                    });
                    auth.clone()
                };
                let storage_warning =
                    save_auth_if_current(state, pending.generation, auth_snapshot).await?;
                ensure_auth_generation_is_current(state, pending.generation)?;
                state.auth_status(
                    TwitchStatusDomain::Auth,
                    TwitchStatus::Connected,
                    Some(format!(
                        "Twitch に {} としてログインしました。",
                        profile.login
                    )),
                );
                state.auth_log(
                    AppLogLevel::Info,
                    format!("Twitch に {} としてログインしました。", profile.login),
                );
                Ok(TwitchAuthPollResult::Authorized {
                    profile,
                    storage_warning,
                })
            }
            Err(PollAuthError::Pending) => Ok(TwitchAuthPollResult::Pending {
                message: {
                    ensure_pending_auth_is_current(state, pending.generation)?;
                    clear_poll_in_flight_if_current(state, pending.generation)?;
                    state.auth_status(
                        TwitchStatusDomain::Auth,
                        TwitchStatus::Connecting,
                        Some("Twitch の認可完了を待っています。".to_string()),
                    );
                    "Twitch の認可完了を待っています。ブラウザでコードを入力してください。"
                        .to_string()
                },
                interval: pending.interval,
            }),
            Err(PollAuthError::SlowDown) => {
                let interval = pending.interval + 5;
                let mut auth = state.auth().lock().map_err(|error| error.to_string())?;
                if auth.generation != pending.generation {
                    return Err(
                        "新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。"
                            .to_string(),
                    );
                }
                if let Some(stored) = &mut auth.pending {
                    stored.interval = interval;
                    stored.poll_in_flight = false;
                }
                state.auth_status(
                    TwitchStatusDomain::Auth,
                    TwitchStatus::Connecting,
                    Some("Twitch 認証の確認間隔を延長しました。".to_string()),
                );
                Ok(TwitchAuthPollResult::SlowDown {
                    message: "確認間隔が短すぎます。少し待ってから再確認してください。".to_string(),
                    interval,
                })
            }
            Err(PollAuthError::Denied) => {
                ensure_pending_auth_is_current(state, pending.generation)?;
                clear_pending_if_current(state, pending.generation)?;
                Ok(TwitchAuthPollResult::Denied {
                    message: {
                        state.auth_status(
                            TwitchStatusDomain::Auth,
                            TwitchStatus::AuthRequired,
                            Some("Twitch 認証がキャンセルされました。".to_string()),
                        );
                        state.auth_log(
                            AppLogLevel::Warning,
                            "Twitch 認証がキャンセルされました。必要なら再度開始してください。",
                        );
                        "Twitch 認証がキャンセルされました。必要なら再度開始してください。"
                            .to_string()
                    },
                })
            }
            Err(PollAuthError::Expired) => {
                ensure_pending_auth_is_current(state, pending.generation)?;
                clear_pending_if_current(state, pending.generation)?;
                Ok(TwitchAuthPollResult::Expired {
                    message: {
                        state.auth_status(
                            TwitchStatusDomain::Auth,
                            TwitchStatus::AuthRequired,
                            Some("Twitch 認証コードの期限が切れました。".to_string()),
                        );
                        state.auth_log(
                            AppLogLevel::Warning,
                            "Twitch 認証コードの期限が切れました。再度開始してください。",
                        );
                        "Twitch 認証コードの期限が切れました。再度開始してください。".to_string()
                    },
                })
            }
            Err(PollAuthError::Other(error)) => {
                ensure_pending_auth_is_current(state, pending.generation)?;
                clear_poll_in_flight_if_current(state, pending.generation)?;
                let message = to_twitch_user_message(error);
                state.auth_status(
                    TwitchStatusDomain::Auth,
                    TwitchStatus::Error,
                    Some(message.clone()),
                );
                state.auth_log(AppLogLevel::Error, message.clone());
                Err(message)
            }
        }
    }
    pub(super) async fn validate(&self) -> Result<TwitchAuthValidationResult, String> {
        let state = self.runtime;
        // Validation and EventSub refresh share this lock across the network
        // request and commit, so a delayed response cannot race token rotation.
        let _credential_update = state.store().lock_credential_update().await;
        let (generation, credential_revision, access_token, refresh_token, client_id) = {
            let auth = state.auth().lock().map_err(|error| error.to_string())?;
            let token = auth
                .token
                .as_ref()
                .ok_or_else(|| "Twitch にログインしていません。".to_string())?;
            let client_id = auth
                .profile
                .as_ref()
                .map(|profile| profile.client_id.clone())
                .filter(|client_id| !client_id.trim().is_empty())
                .or_else(|| Some(state.client_id()))
                .unwrap_or_default();
            (
                auth.generation,
                auth.credential_revision,
                token.access_token.clone(),
                token.refresh_token.clone(),
                client_id,
            )
        };

        let profile = match state.validate(&access_token).await {
            Ok(validate) => {
                let profile = TwitchUserProfile::from(validate);
                if let Err(error) = ensure_required_twitch_scopes(&profile.scopes) {
                    let message = error.to_string();
                    if clear_credentials_if_current(
                        state,
                        generation,
                        credential_revision,
                        &access_token,
                        &refresh_token,
                        &message,
                        true,
                    )
                    .await?
                    {
                        return Err(message);
                    }
                    return Err(stale_credential_message());
                }
                profile
            }
            Err(validate_error) => {
                let credentials = EventSubAuthCredentials {
                    generation,
                    credential_revision,
                    client_id,
                    access_token: access_token.clone(),
                    refresh_token: refresh_token.clone(),
                };
                match refresh_credentials_if_current(state, &credentials).await {
                    Ok((profile, storage_warning)) => {
                        state.auth_status(
                            TwitchStatusDomain::Auth,
                            TwitchStatus::Connected,
                            Some("Twitch 認証を更新しました。".to_string()),
                        );
                        state.auth_log(AppLogLevel::Info, "Twitch 認証を更新しました。");
                        return Ok(TwitchAuthValidationResult {
                            profile,
                            storage_warning,
                        });
                    }
                    Err(error) if error.is::<StaleCredentialResponse>() => {
                        return Err(stale_credential_message());
                    }
                    Err(error) => {
                        let missing_scope = error
                            .downcast_ref::<MissingTwitchScope>()
                            .map(|error| error.0.clone());
                        let definitive = missing_scope.is_none()
                            && error
                                .downcast_ref::<super::error::TwitchApiError>()
                                .is_some_and(|api_error| api_error.auth_failure().is_some());
                        if let Some(message) = missing_scope {
                            if clear_credentials_if_current(
                                state,
                                generation,
                                credential_revision,
                                &access_token,
                                &refresh_token,
                                &message,
                                true,
                            )
                            .await?
                            {
                                return Err(message);
                            }
                            return Err(stale_credential_message());
                        }
                        if definitive {
                            let message = to_twitch_user_message(error);
                            if clear_credentials_if_current(
                                state,
                                generation,
                                credential_revision,
                                &access_token,
                                &refresh_token,
                                &message,
                                false,
                            )
                            .await?
                            {
                                return Err(message);
                            }
                            return Err(stale_credential_message());
                        }
                        let error = if error
                            .downcast_ref::<super::error::TwitchApiError>()
                            .is_some()
                        {
                            error
                        } else {
                            anyhow::anyhow!("{validate_error}; {error}")
                        };
                        return Err(retryable_auth_error_message(&error));
                    }
                }
            }
        };

        let auth_snapshot = {
            let mut auth = state.auth().lock().map_err(|error| error.to_string())?;
            if !auth.credentials_match(credential_revision, &access_token, &refresh_token) {
                return Err(stale_credential_message());
            }
            apply_validated_profile(&mut auth, generation, profile.clone())?;
            auth.clone()
        };
        let storage_warning = save_auth_if_current(state, generation, auth_snapshot).await?;
        ensure_auth_generation_is_current(state, generation)?;
        state.auth_status(
            TwitchStatusDomain::Auth,
            TwitchStatus::Connected,
            Some("Twitch 認証は有効です。".to_string()),
        );
        state.auth_log(AppLogLevel::Info, "Twitch 認証は有効です。");
        Ok(TwitchAuthValidationResult {
            profile,
            storage_warning,
        })
    }
}

pub(super) fn apply_validated_profile(
    auth: &mut TwitchAuthState,
    generation: u64,
    profile: TwitchUserProfile,
) -> Result<(), String> {
    if auth.generation != generation {
        return Err(
            "新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。".to_string(),
        );
    }
    auth.profile = Some(profile);
    Ok(())
}

pub(super) fn stored_auth_profile(
    auth: &std::sync::Mutex<TwitchAuthState>,
) -> Result<Option<TwitchUserProfile>, String> {
    Ok(auth.lock().map_err(|error| error.to_string())?.profile())
}

pub(super) async fn clear_twitch_auth_state(state: &impl AuthRuntime) -> Result<(), String> {
    clear_twitch_auth_state_with_store(state.auth().clone(), state.store()).await?;

    state.cancel_chat()?;
    Ok(())
}

/// Clears durable Twitch credentials after first invalidating the in-memory
/// generation. This is kept separate from the Tauri command wiring so the same
/// production path can be exercised with a delayed credential-store backend.
pub(super) async fn clear_twitch_auth_state_with_store(
    auth_state: std::sync::Arc<std::sync::Mutex<TwitchAuthState>>,
    store: &TwitchAuthStore,
) -> Result<(), String> {
    // Invalidate the in-memory generation before waiting for the store. This
    // prevents an in-flight refresh or validation from publishing success while
    // logout waits for the shared credential-update lock.
    let (previous_auth, generation, credential_revision) = {
        let mut auth = auth_state.lock().map_err(|error| error.to_string())?;
        let previous_auth = auth.clone();
        let generation = auth.invalidate_operations();
        auth.credential_revision = auth.credential_revision.wrapping_add(1);
        auth.token = None;
        auth.profile = None;
        (previous_auth, generation, auth.credential_revision)
    };

    let _credential_update = store.lock_credential_update().await;
    match store
        .clear_if_current(auth_state.clone(), generation, credential_revision)
        .await
    {
        Ok(AuthClearOutcome::Cleared) => Ok(()),
        Ok(AuthClearOutcome::Stale | AuthClearOutcome::StaleAfterClear) => {
            Err("新しい Twitch 認証操作が開始されたため、古い解除結果を破棄しました。".to_string())
        }
        Err(error) => {
            let mut auth = auth_state.lock().map_err(|error| error.to_string())?;
            restore_auth_after_failed_clear_if_current(
                &mut auth,
                generation,
                credential_revision,
                previous_auth,
            );
            Err(to_secure_store_user_message(error))
        }
    }
}

pub(super) fn restore_auth_after_failed_clear_if_current(
    auth: &mut TwitchAuthState,
    generation: u64,
    credential_revision: u64,
    mut previous_auth: TwitchAuthState,
) {
    if auth.generation == generation && auth.credential_revision == credential_revision {
        // Restoring a credential after a failed delete must not roll back the
        // generation. Older poll/validate/save operations remain stale even
        // though the user can continue using the prior credential.
        previous_auth.generation = generation;
        previous_auth.credential_revision = credential_revision;
        previous_auth.pending = None;
        *auth = previous_auth;
    }
}

pub(super) fn ensure_pending_auth_is_current(
    state: &impl AuthRuntime,
    generation: u64,
) -> Result<(), String> {
    let auth = state.auth().lock().map_err(|error| error.to_string())?;
    if auth.pending_is_current(generation) {
        Ok(())
    } else {
        Err("新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。".to_string())
    }
}

pub(super) fn ensure_auth_generation_is_current(
    state: &impl AuthRuntime,
    generation: u64,
) -> Result<(), String> {
    let auth = state.auth().lock().map_err(|error| error.to_string())?;
    if auth.generation == generation {
        Ok(())
    } else {
        Err("新しい Twitch 認証操作が開始されたため、古い確認結果を破棄しました。".to_string())
    }
}

pub(super) fn clear_poll_in_flight_if_current(
    state: &impl AuthRuntime,
    generation: u64,
) -> Result<(), String> {
    let mut auth = state.auth().lock().map_err(|error| error.to_string())?;
    if auth.generation == generation {
        if let Some(pending) = &mut auth.pending {
            pending.poll_in_flight = false;
        }
    }
    Ok(())
}

pub(super) fn clear_pending_if_current(
    state: &impl AuthRuntime,
    generation: u64,
) -> Result<(), String> {
    let mut auth = state.auth().lock().map_err(|error| error.to_string())?;
    if auth.generation == generation {
        auth.pending = None;
    }
    Ok(())
}

pub(super) async fn clear_missing_scope_twitch_auth(
    state: &impl AuthRuntime,
    error_message: &str,
) -> Result<(), String> {
    clear_twitch_auth_state(state).await?;
    state.require_auth(
        TwitchAuthRequiredReason::MissingRequiredScope,
        error_message,
    );
    state.auth_log(AppLogLevel::Warning, error_message);
    Ok(())
}

pub(super) async fn save_auth_if_current(
    state: &impl AuthRuntime,
    generation: u64,
    auth: TwitchAuthState,
) -> Result<Option<String>, String> {
    match state
        .store()
        .save_if_current(state.auth().clone(), generation, auth)
        .await
        .map_err(to_secure_store_user_message)?
    {
        AuthSaveOutcome::Saved(warning) => Ok(warning),
        AuthSaveOutcome::Stale => {
            Err("新しい Twitch 認証操作が開始されたため、古い保存結果を破棄しました。".to_string())
        }
    }
}

pub(super) fn stale_credential_message() -> String {
    "新しい Twitch 認証情報が反映されたため、古い応答を破棄しました。".to_string()
}

pub(super) fn credentials_are_current(
    state: &impl AuthRuntime,
    generation: u64,
    credential_revision: u64,
    access_token: &str,
    refresh_token: &str,
) -> Result<bool, String> {
    let auth = state.auth().lock().map_err(|error| error.to_string())?;
    Ok(auth.generation == generation
        && auth.credentials_match(credential_revision, access_token, refresh_token))
}

/// Clear only the exact credential revision whose request failed. Both the
/// in-memory invalidation and durable clear are guarded by generation/revision.
pub(super) async fn clear_credentials_if_current(
    state: &impl AuthRuntime,
    expected_generation: u64,
    expected_revision: u64,
    expected_access_token: &str,
    expected_refresh_token: &str,
    error_message: &str,
    missing_scope: bool,
) -> Result<bool, String> {
    let (previous_auth, generation, credential_revision) = {
        let mut auth = state.auth().lock().map_err(|error| error.to_string())?;
        if auth.generation != expected_generation
            || !auth.credentials_match(
                expected_revision,
                expected_access_token,
                expected_refresh_token,
            )
        {
            return Ok(false);
        }
        let previous = auth.clone();
        let generation = auth.invalidate_operations();
        auth.credential_revision = auth.credential_revision.wrapping_add(1);
        auth.token = None;
        auth.profile = None;
        (previous, generation, auth.credential_revision)
    };

    match state
        .store()
        .clear_if_current(state.auth().clone(), generation, credential_revision)
        .await
        .map_err(to_secure_store_user_message)
    {
        Ok(AuthClearOutcome::Cleared) => {}
        Ok(AuthClearOutcome::Stale | AuthClearOutcome::StaleAfterClear) => return Ok(false),
        Err(error) => {
            let mut auth = state.auth().lock().map_err(|error| error.to_string())?;
            restore_auth_after_failed_clear_if_current(
                &mut auth,
                generation,
                credential_revision,
                previous_auth,
            );
            return Err(error);
        }
    }

    state.cancel_chat()?;
    if missing_scope {
        state.require_auth(
            TwitchAuthRequiredReason::MissingRequiredScope,
            error_message,
        );
    } else {
        let message = format!("Twitch 認証が無効なため、認証状態を解除しました: {error_message}");
        state.auth_status(
            TwitchStatusDomain::Auth,
            TwitchStatus::AuthRequired,
            Some(message.clone()),
        );
        state.auth_log(AppLogLevel::Warning, message);
        return Ok(true);
    }
    state.auth_log(AppLogLevel::Warning, error_message);
    Ok(true)
}
