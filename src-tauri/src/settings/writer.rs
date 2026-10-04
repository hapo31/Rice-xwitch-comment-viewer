//! One process owns this settings directory for its entire lifetime.
//! The stable lock file is never replaced or deleted: locking settings.json
//! itself would lose ownership at the first atomic replacement.
use super::{protect_metadata, protect_storage};
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
#[cfg(feature = "app")]
use tauri::Manager;

#[derive(Debug)]
struct SettingsWriterGuard {
    settings_path: PathBuf,
    _file: File,
}

impl SettingsWriterGuard {
    fn acquire(settings_path: &Path) -> anyhow::Result<Self> {
        let lock_path = settings_path.with_file_name("settings.writer.lock");
        // Harden only the lock and app-specific directory before ownership;
        // do not read, quarantine or alter settings/backup in a second process.
        protect_storage(&lock_path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let file = options.open(&lock_path)?;
        let metadata = file.metadata()?;
        anyhow::ensure!(
            metadata.is_file(),
            "設定のロックファイルが通常のファイルではありません。"
        );
        protect_metadata(&lock_path, &metadata, 0o600)?;
        match file.try_lock() {
            Ok(()) => Ok(Self { settings_path: settings_path.to_path_buf(), _file: file }),
            Err(TryLockError::WouldBlock) => Err(anyhow::anyhow!(
                "Rice は既に起動しています。先に起動したウィンドウを使用してください。"
            )),
            Err(TryLockError::Error(error)) => Err(anyhow::anyhow!(
                "設定の書込み所有権を取得できません。保存場所とアクセス権を確認してください: {error}"
            )),
        }
    }

    fn require_path(&self, path: &Path) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.settings_path == path,
            "設定の書込み所有権が保存先と一致しません。"
        );
        Ok(())
    }
}

#[cfg(feature = "app")]
pub(super) fn initialize<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    path: &Path,
) -> anyhow::Result<()> {
    if let Some(guard) = app.try_state::<SettingsWriterGuard>() {
        return guard.require_path(path);
    }
    let guard = SettingsWriterGuard::acquire(path)?;
    anyhow::ensure!(
        app.manage(guard),
        "設定の書込み所有権は既に登録されています。"
    );
    require_owned(app, path)
}

#[cfg(feature = "app")]
pub(super) fn require_owned<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    path: &Path,
) -> anyhow::Result<()> {
    app.try_state::<SettingsWriterGuard>()
        .ok_or_else(|| {
            anyhow::anyhow!("設定の書込み所有権がありません。Rice を起動し直してください。")
        })?
        .require_path(path)
}

#[cfg(test)]
mod tests;
