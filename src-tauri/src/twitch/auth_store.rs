//! Twitch auth_store responsibility boundary.
use super::auth_state::{MissingRequiredTwitchScopes, StoredTwitchAuth, TwitchAuthState};
use super::error::{
    to_auth_recovery_failure_user_message, to_legacy_cleanup_user_message,
    to_secure_store_load_user_message, to_session_only_user_message,
};
use super::{KEYRING_ACCOUNT, KEYRING_SERVICE};
#[cfg(target_os = "linux")]
use super::{LEGACY_AUTH_DIR, LEGACY_AUTH_FILE};
#[cfg(target_os = "linux")]
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub(super) trait AuthSecretStore {
    fn load_secret(&self) -> anyhow::Result<Option<String>>;
    fn save_secret(&self, secret: &str) -> anyhow::Result<()>;
    fn clear_secret(&self) -> anyhow::Result<()>;
}

pub(super) struct AuthStorage<'a, SecureStore, LegacyStore> {
    pub(super) secure: &'a SecureStore,
    pub(super) legacy: &'a LegacyStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthLoadReason {
    MissingRequiredScope,
    StoreUnavailable,
    CorruptData,
    LegacyMigrated,
    LegacyCleanupFailed,
}

/// Display text never controls the startup authentication transition.
pub(crate) struct AuthLoadNotice {
    pub(crate) reason: AuthLoadReason,
    pub(crate) message: String,
}

impl AuthLoadNotice {
    fn new(reason: AuthLoadReason, message: String) -> Self {
        Self { reason, message }
    }
}

pub(crate) struct AuthLoadResult {
    pub(crate) auth: Option<TwitchAuthState>,
    pub(crate) notice: Option<AuthLoadNotice>,
}

impl<SecureStore: AuthSecretStore, LegacyStore: AuthSecretStore>
    AuthStorage<'_, SecureStore, LegacyStore>
{
    pub(super) fn load(&self) -> AuthLoadResult {
        match self.secure.load_secret() {
            Ok(Some(secret)) => match restore_stored_auth(&secret) {
                Ok(auth) => AuthLoadResult {
                    auth: Some(auth),
                    notice: self.legacy.clear_secret().err().map(|error| {
                        AuthLoadNotice::new(AuthLoadReason::LegacyCleanupFailed, to_legacy_cleanup_user_message(error))
                    }),
                },
                Err(error) => AuthLoadResult {
                    auth: None,
                    notice: Some(AuthLoadNotice::new(error.reason(), format!(
                        "OS の資格情報ストアにある Twitch 認証情報を読み込めませんでした。Login から再認証してください: {error}"
                    ))),
                },
            },
            Ok(None) => self.migrate_legacy_auth(None),
            Err(error) => self.migrate_legacy_auth(Some(error)),
        }
    }

    pub(super) fn migrate_legacy_auth(
        &self,
        secure_load_error: Option<anyhow::Error>,
    ) -> AuthLoadResult {
        let secret = match self.legacy.load_secret() {
            Ok(Some(secret)) => secret,
            Ok(None) => {
                return AuthLoadResult {
                    auth: None,
                    notice: secure_load_error.map(|error| {
                        AuthLoadNotice::new(
                            AuthLoadReason::StoreUnavailable,
                            to_secure_store_load_user_message(error),
                        )
                    }),
                }
            }
            Err(error) => {
                return AuthLoadResult {
                    auth: None,
                    notice: Some(AuthLoadNotice::new(
                        AuthLoadReason::StoreUnavailable,
                        to_auth_recovery_failure_user_message(secure_load_error, error),
                    )),
                }
            }
        };

        let auth = match restore_stored_auth(&secret) {
            Ok(auth) => auth,
            Err(error) => {
                return AuthLoadResult {
                    auth: None,
                    notice: Some(AuthLoadNotice::new(
                        error.reason(),
                        to_auth_recovery_failure_user_message(secure_load_error, error.into()),
                    )),
                }
            }
        };

        match self.secure.save_secret(&secret) {
            Ok(()) => AuthLoadResult {
                auth: Some(auth),
                notice: Some(match self.legacy.clear_secret() {
                    Ok(()) => AuthLoadNotice::new(AuthLoadReason::LegacyMigrated,
                        "以前のローカル認証情報を OS の資格情報ストアへ移行し、平文ファイルを削除しました。".to_string()),
                    Err(error) => AuthLoadNotice::new(AuthLoadReason::LegacyCleanupFailed, format!(
                        "以前のローカル認証情報を OS の資格情報ストアへ移行しましたが、平文ファイルを削除できませんでした。{}",
                        to_legacy_cleanup_user_message(error))),
                }),
            },
            Err(error) => AuthLoadResult {
                auth: None,
                notice: Some(AuthLoadNotice::new(AuthLoadReason::StoreUnavailable,
                    to_auth_recovery_failure_user_message(secure_load_error, error))),
            },
        }
    }

    pub(super) fn save(&self, auth: &TwitchAuthState) -> anyhow::Result<Option<String>> {
        let stored = auth
            .stored_auth()
            .ok_or_else(|| anyhow::anyhow!("保存できる Twitch 認証状態がありません。"))?;
        let secret = serde_json::to_string(&stored)?;

        match self.secure.save_secret(&secret) {
            Ok(()) => Ok(self
                .legacy
                .clear_secret()
                .err()
                .map(to_legacy_cleanup_user_message)),
            Err(error) => Ok(Some(to_session_only_user_message(error))),
        }
    }

    pub(super) fn clear(&self) -> anyhow::Result<()> {
        let secure_result = self.secure.clear_secret();
        let legacy_result = self.legacy.clear_secret();
        match (secure_result, legacy_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Err(secure_error), Err(legacy_error)) => {
                Err(anyhow::anyhow!("{secure_error}; {legacy_error}"))
            }
        }
    }
}

pub(crate) trait AuthCredentialStore: Send + Sync {
    fn load(&self) -> AuthLoadResult;
    fn save(&self, auth: &TwitchAuthState) -> anyhow::Result<Option<String>>;
    fn clear(&self) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub(crate) struct TwitchAuthStore {
    backend: std::sync::Arc<dyn AuthCredentialStore>,
    io_lock: std::sync::Arc<std::sync::Mutex<()>>,
    credential_update_lock: std::sync::Arc<tokio::sync::Mutex<()>>,
}

impl Default for TwitchAuthStore {
    fn default() -> Self {
        Self::with_backend(std::sync::Arc::new(SystemAuthCredentialStore))
    }
}

impl TwitchAuthStore {
    pub(crate) fn with_backend(backend: std::sync::Arc<dyn AuthCredentialStore>) -> Self {
        Self {
            backend,
            io_lock: std::sync::Arc::new(std::sync::Mutex::new(())),
            credential_update_lock: std::sync::Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub(super) async fn lock_credential_update(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.credential_update_lock.clone().lock_owned().await
    }

    pub(crate) async fn load(&self) -> anyhow::Result<AuthLoadResult> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.load_sync())
            .await
            .map_err(anyhow::Error::from)
    }

    pub(super) fn load_sync(&self) -> AuthLoadResult {
        let _io_guard = self.io_lock.lock().expect("auth storage mutex poisoned");
        self.backend.load()
    }

    pub(super) async fn save_if_current(
        &self,
        auth_state: std::sync::Arc<std::sync::Mutex<TwitchAuthState>>,
        generation: u64,
        auth: TwitchAuthState,
    ) -> anyhow::Result<AuthSaveOutcome> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.save_if_current_sync(&auth_state, generation, &auth)
        })
        .await
        .map_err(anyhow::Error::from)?
    }

    pub(super) fn save_if_current_sync(
        &self,
        auth_state: &std::sync::Mutex<TwitchAuthState>,
        generation: u64,
        auth: &TwitchAuthState,
    ) -> anyhow::Result<AuthSaveOutcome> {
        // Serialize save and clear operations. The generation check is made
        // after acquiring this lock, so a save queued behind logout cannot
        // resurrect credentials that logout has already removed.
        let _io_guard = self
            .io_lock
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let is_current = {
            let current = auth_state
                .lock()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            current.generation == generation
                && current.credential_revision == auth.credential_revision
        };
        if !is_current {
            return Ok(AuthSaveOutcome::Stale);
        }
        let warning = self.backend.save(auth)?;
        let is_current = {
            let current = auth_state
                .lock()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            current.generation == generation
                && current.credential_revision == auth.credential_revision
        };
        if is_current {
            Ok(AuthSaveOutcome::Saved(warning))
        } else {
            Ok(AuthSaveOutcome::Stale)
        }
    }

    pub(super) async fn clear_if_current(
        &self,
        auth_state: std::sync::Arc<std::sync::Mutex<TwitchAuthState>>,
        generation: u64,
        credential_revision: u64,
    ) -> anyhow::Result<AuthClearOutcome> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.clear_if_current_sync(&auth_state, generation, credential_revision)
        })
        .await
        .map_err(anyhow::Error::from)?
    }

    pub(super) fn clear_if_current_sync(
        &self,
        auth_state: &std::sync::Mutex<TwitchAuthState>,
        generation: u64,
        credential_revision: u64,
    ) -> anyhow::Result<AuthClearOutcome> {
        let _io_guard = self
            .io_lock
            .lock()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let is_current = {
            let current = auth_state
                .lock()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            current.generation == generation && current.credential_revision == credential_revision
        };
        if !is_current {
            return Ok(AuthClearOutcome::Stale);
        }
        self.backend.clear()?;
        // The backend call can block after the pre-clear comparison. Keep the
        // I/O lock while checking once more so a newer auth/save waits to write
        // after this old clear, and callers never tear down its connection.
        let is_current = {
            let current = auth_state
                .lock()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            current.generation == generation && current.credential_revision == credential_revision
        };
        if !is_current {
            Ok(AuthClearOutcome::StaleAfterClear)
        } else {
            Ok(AuthClearOutcome::Cleared)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AuthSaveOutcome {
    Saved(Option<String>),
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AuthClearOutcome {
    Cleared,
    Stale,
    StaleAfterClear,
}

pub(super) struct SystemAuthCredentialStore;

impl AuthCredentialStore for SystemAuthCredentialStore {
    fn load(&self) -> AuthLoadResult {
        AuthStorage {
            secure: &SYSTEM_KEYRING_STORE,
            legacy: &LegacyAuthStore,
        }
        .load()
    }

    fn save(&self, auth: &TwitchAuthState) -> anyhow::Result<Option<String>> {
        AuthStorage {
            secure: &SYSTEM_KEYRING_STORE,
            legacy: &LegacyAuthStore,
        }
        .save(auth)
    }

    fn clear(&self) -> anyhow::Result<()> {
        AuthStorage {
            secure: &SYSTEM_KEYRING_STORE,
            legacy: &LegacyAuthStore,
        }
        .clear()
    }
}

pub(super) struct KeyringAuthStore<'a> {
    pub(super) service: &'a str,
    pub(super) account: &'a str,
}

const SYSTEM_KEYRING_STORE: KeyringAuthStore<'static> = KeyringAuthStore {
    service: KEYRING_SERVICE,
    account: KEYRING_ACCOUNT,
};

impl AuthSecretStore for KeyringAuthStore<'_> {
    fn load_secret(&self) -> anyhow::Result<Option<String>> {
        let entry = self.entry()?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn save_secret(&self, secret: &str) -> anyhow::Result<()> {
        self.entry()?.set_password(secret)?;
        Ok(())
    }

    fn clear_secret(&self) -> anyhow::Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl KeyringAuthStore<'_> {
    pub(super) fn entry(&self) -> anyhow::Result<keyring::Entry> {
        Ok(keyring::Entry::new(self.service, self.account)?)
    }
}

pub(super) struct LegacyAuthStore;

#[cfg(all(feature = "app", target_os = "linux"))]
impl AuthSecretStore for LegacyAuthStore {
    fn load_secret(&self) -> anyhow::Result<Option<String>> {
        load_legacy_auth_secret()
    }

    fn save_secret(&self, _secret: &str) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("平文の認証情報ファイルは作成しません。"))
    }

    fn clear_secret(&self) -> anyhow::Result<()> {
        clear_legacy_auth()
    }
}

#[cfg(all(feature = "app", not(target_os = "linux")))]
impl AuthSecretStore for LegacyAuthStore {
    fn load_secret(&self) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    fn save_secret(&self, _secret: &str) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("平文の認証情報ファイルは作成しません。"))
    }

    fn clear_secret(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum StoredAuthRestoreError {
    // Serde errors can contain the invalid input value, including a token.
    #[error("保存済み認証情報の形式が不正です。")]
    CorruptData,
    #[error(transparent)]
    MissingRequiredScope(#[from] MissingRequiredTwitchScopes),
}

impl StoredAuthRestoreError {
    fn reason(&self) -> AuthLoadReason {
        match self {
            Self::CorruptData => AuthLoadReason::CorruptData,
            Self::MissingRequiredScope(_) => AuthLoadReason::MissingRequiredScope,
        }
    }
}

pub(super) fn restore_stored_auth(secret: &str) -> Result<TwitchAuthState, StoredAuthRestoreError> {
    let stored = serde_json::from_str::<StoredTwitchAuth>(secret)
        .map_err(|_| StoredAuthRestoreError::CorruptData)?;
    TwitchAuthState::restore(stored).map_err(Into::into)
}

#[cfg(all(feature = "app", target_os = "linux"))]
pub(super) fn load_legacy_auth_secret() -> anyhow::Result<Option<String>> {
    let path = match legacy_auth_path() {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };
    if !path.exists() {
        return Ok(None);
    }

    ensure_legacy_permissions(&path)?;
    Ok(Some(fs::read_to_string(path)?))
}

#[cfg(all(feature = "app", target_os = "linux"))]
pub(super) fn clear_legacy_auth() -> anyhow::Result<()> {
    let path = match legacy_auth_path() {
        Ok(path) => path,
        Err(_) => return Ok(()),
    };
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(all(feature = "app", target_os = "linux"))]
pub(super) fn legacy_auth_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        anyhow::anyhow!("HOME が設定されていないため、Twitch 認証情報を保存できません。")
    })?;
    Ok(PathBuf::from(home)
        .join(LEGACY_AUTH_DIR)
        .join(LEGACY_AUTH_FILE))
}

#[cfg(all(feature = "app", target_os = "linux"))]
pub(super) fn ensure_legacy_parent_permissions(path: &Path) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Twitch 認証情報の保存先ディレクトリが見つかりません。"))?;
    if parent.exists() {
        if !parent.is_dir() {
            return Err(anyhow::anyhow!(
                "Twitch 認証情報の保存先がディレクトリではありません: {}",
                parent.display()
            ));
        }
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        return Ok(());
    }

    fs::DirBuilder::new().mode(0o700).create(parent)?;
    Ok(())
}

#[cfg(all(feature = "app", target_os = "linux"))]
pub(super) fn ensure_legacy_permissions(path: &Path) -> anyhow::Result<()> {
    ensure_legacy_parent_permissions(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}
