use super::{validation, AppSettings};
use crate::launcher::validate_launcher_resources;
use crate::resource_limits::{
    check_bytes, read_bounded, serialize_bounded, SizeLimitExceeded, MAX_SETTINGS_JSON_BYTES,
};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "app")]
use tauri::Manager;

use super::schema;
#[cfg(feature = "app")]
use super::writer;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "app")]
fn settings_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join("settings.json"))
}

pub struct SettingsStore;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct SettingsRecoveryNotice {
    pub message: String,
}

#[derive(Debug)]
pub struct LoadedSettings {
    pub settings: AppSettings,
    pub recovery_notice: Option<SettingsRecoveryNotice>,
}

fn read_settings_text(path: &Path) -> anyhow::Result<Result<String, String>> {
    match read_bounded(path, MAX_SETTINGS_JSON_BYTES) {
        Ok(text) => Ok(Ok(text)),
        Err(error) if error.downcast_ref::<SizeLimitExceeded>().is_some() => {
            Ok(Err(error.to_string()))
        }
        // IO/permission failures are not evidence of corrupt content. Fail closed.
        Err(error) => Err(error),
    }
}

impl SettingsStore {
    #[cfg(feature = "app")]
    pub fn load<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<LoadedSettings> {
        let path = settings_path(app)?;
        writer::initialize(app, &path)?;
        Self::load_from_path(&path)
    }

    pub(super) fn load_from_path(path: &Path) -> anyhow::Result<LoadedSettings> {
        protect_storage(path)?;
        protect_existing_file(&backup_path(path))?;
        if !path.exists() {
            let settings = AppSettings::default();
            Self::save_to_path(path, &settings)?;
            return Ok(LoadedSettings {
                settings,
                recovery_notice: None,
            });
        }

        let loaded = read_settings_text(path)?.and_then(|text| schema::decode(&text));
        match loaded {
            Ok(decoded) => {
                if decoded.needs_resave {
                    Self::save_to_path(path, &decoded.settings)?;
                }
                Ok(LoadedSettings {
                    settings: decoded.settings,
                    // Ordinary optional/invalid-field fallback is silent by owner policy.
                    // Only an unsupported version needs the safe recovery instruction.
                    recovery_notice: decoded.unsupported_version.then(|| SettingsRecoveryNotice {
                        message: schema::READ_ONLY_MESSAGE.into(),
                    }),
                })
            }
            Err(reason) => Self::recover_from_invalid_primary(path, &reason),
        }
    }

    #[cfg(feature = "app")]
    pub fn save<R: tauri::Runtime>(
        app: &tauri::AppHandle<R>,
        settings: &AppSettings,
    ) -> anyhow::Result<()> {
        let path = settings_path(app)?;
        writer::require_owned(app, &path)?;
        Self::save_to_path(&path, settings)
    }

    pub(super) fn save_to_path(path: &Path, settings: &AppSettings) -> anyhow::Result<()> {
        Self::save_to_path_with_fault(path, settings, SaveFault::None)
    }

    pub(super) fn save_to_path_with_fault(
        path: &Path,
        settings: &AppSettings,
        fault: SaveFault,
    ) -> anyhow::Result<()> {
        validation::validate_settings(settings)?;
        validate_launcher_resources(&settings.launcher.items).map_err(anyhow::Error::msg)?;
        let bytes = serialize_bounded(
            &schema::PersistedSettings::new(settings),
            MAX_SETTINGS_JSON_BYTES,
            "設定JSON",
        )?;
        let text = std::str::from_utf8(&bytes)?;
        Self::save_text_to_path(path, text, fault)
    }

    pub(super) fn save_text_to_path(
        path: &Path,
        text: &str,
        fault: SaveFault,
    ) -> anyhow::Result<()> {
        check_bytes(text.len(), MAX_SETTINGS_JSON_BYTES, "設定JSON")?;
        protect_storage(path)?;
        protect_existing_file(&backup_path(path))?;

        // Check before creating temporary files or rotating backup. This same
        // guard covers Settings, Launcher and best-effort window/exit saves,
        // including a future document installed after this process loaded.
        let previous = if path.exists() {
            let previous = read_bounded(path, MAX_SETTINGS_JSON_BYTES)?;
            let decoded = schema::decode(&previous).map_err(|error| {
                anyhow::anyhow!("既存の設定をバックアップできませんでした: {error}")
            })?;
            if decoded.read_only {
                return Err(schema::ReadOnlySettings.into());
            }
            Some(previous)
        } else {
            None
        };
        let temporary_path = write_temp_file(path, text.as_bytes(), fault)?;
        let result = (|| {
            if let Some(previous) = previous {
                atomic_write(&backup_path(path), previous.as_bytes(), SaveFault::None)?;
            }
            replace_file(&temporary_path, path, fault)?;
            sync_parent_directory(path)?;
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        result
    }

    fn recover_from_invalid_primary(
        path: &Path,
        primary_reason: &str,
    ) -> anyhow::Result<LoadedSettings> {
        let corrupted_primary = quarantine_file(path)?;
        let backup = backup_path(path);

        if backup.exists() {
            let backup_reason = match read_settings_text(&backup)? {
                Ok(backup_text) => match schema::decode(&backup_text) {
                    Ok(decoded) => {
                        atomic_write(path, backup_text.as_bytes(), SaveFault::None)?;
                        if decoded.needs_resave {
                            Self::save_to_path(path, &decoded.settings)?;
                        }
                        let message = format!(
                            "設定ファイルの内容が無効（{primary_reason}）だったため、バックアップから復旧しました。退避先: {}",
                            corrupted_primary.display()
                        );
                        return Ok(LoadedSettings {
                            settings: decoded.settings,
                            recovery_notice: Some(SettingsRecoveryNotice {
                                message: if decoded.read_only {
                                    format!("{message} {}", schema::READ_ONLY_MESSAGE)
                                } else {
                                    message
                                },
                            }),
                        });
                    }
                    Err(reason) => reason,
                },
                Err(reason) => reason,
            };
            let corrupted_backup = quarantine_file(&backup)?;
            let settings = AppSettings::default();
            Self::save_to_path(path, &settings)?;
            return Ok(LoadedSettings {
                settings,
                recovery_notice: Some(SettingsRecoveryNotice {
                    message: format!(
                        "設定ファイルとバックアップの内容が無効だったため、既定値で起動しました。原因: {primary_reason} / {backup_reason} 退避先: {}, {}",
                        corrupted_primary.display(),
                        corrupted_backup.display()
                    ),
                }),
            });
        }

        let settings = AppSettings::default();
        Self::save_to_path(path, &settings)?;
        Ok(LoadedSettings {
            settings,
            recovery_notice: Some(SettingsRecoveryNotice {
                message: format!(
                    "設定ファイルの内容が無効（{primary_reason}）だったため、既定値で起動しました。退避先: {}",
                    corrupted_primary.display()
                ),
            }),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SaveFault {
    None,
    TempWrite,
    Replace,
}

pub(super) fn backup_path(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        "{}.bak",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("settings.json")
    ))
}

fn quarantine_file(path: &Path) -> anyhow::Result<PathBuf> {
    protect_existing_file(path)?;
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S%3f");
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("設定ファイルの親フォルダを取得できません。"))?;

    for suffix in 0..1000_u16 {
        let candidate = parent.join(format!("{file_name}.corrupt-{timestamp}-{suffix}"));
        if !candidate.exists() {
            fs::rename(path, &candidate)?;
            sync_parent_directory(path)?;
            return Ok(candidate);
        }
    }

    Err(anyhow::anyhow!(
        "破損した設定ファイルの退避先を作成できません。"
    ))
}

fn atomic_write(path: &Path, contents: &[u8], fault: SaveFault) -> anyhow::Result<()> {
    check_bytes(contents.len(), MAX_SETTINGS_JSON_BYTES, "設定JSON")?;
    protect_storage(path)?;
    let temporary_path = write_temp_file(path, contents, fault)?;
    let result =
        replace_file(&temporary_path, path, fault).and_then(|_| sync_parent_directory(path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

pub(super) fn write_temp_file(
    path: &Path,
    contents: &[u8],
    fault: SaveFault,
) -> anyhow::Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("設定ファイルの親フォルダを取得できません。"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");

    for _ in 0..1000 {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary_path = parent.join(format!(".{file_name}.{counter}.tmp"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = match options.open(&temporary_path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };

        let result = if fault == SaveFault::TempWrite {
            Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "fault injected: disk full",
            ))
        } else {
            file.write_all(contents).and_then(|_| file.sync_all())
        };
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary_path);
            return Err(error.into());
        }
        return Ok(temporary_path);
    }

    Err(anyhow::anyhow!(
        "設定保存用の一時ファイルを作成できません。"
    ))
}

fn replace_file(source: &Path, destination: &Path, fault: SaveFault) -> anyhow::Result<()> {
    protect_existing_file(source)?;
    protect_existing_file(destination)?;
    if fault == SaveFault::Replace {
        return Err(anyhow::anyhow!("fault injected: atomic replace failed"));
    }

    atomic_replace(source, destination)?;
    protect_existing_file(destination)?;
    Ok(())
}

/// Only the application-specific directory is hardened. Never chmod shared
/// ancestors such as HOME, /tmp, or the platform's app-data root.
pub(super) fn protect_storage(path: &Path) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| anyhow::anyhow!("設定専用フォルダーを取得できません。"))?;
    match fs::symlink_metadata(parent) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(anyhow::anyhow!(
                    "設定フォルダーにリンクや通常以外の種類は使用できません。"
                ));
            }
            protect_metadata(parent, &metadata, 0o700)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(parent)?;
            protect_metadata(parent, &fs::symlink_metadata(parent)?, 0o700)?;
        }
        Err(error) => return Err(error.into()),
    }
    protect_existing_file(path)
}

pub(super) fn protect_existing_file(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(anyhow::anyhow!(
                    "設定ファイルにリンクや通常以外の種類は使用できません。設定の保存場所を確認してください。"
                ));
            }
            protect_metadata(path, &metadata, 0o600)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(unix)]
pub(super) fn validate_owner(metadata: &fs::Metadata, expected: u32) -> anyhow::Result<()> {
    use std::os::unix::fs::MetadataExt;
    if metadata.uid() != expected || (metadata.is_file() && metadata.nlink() != 1) {
        return Err(anyhow::anyhow!(
            "設定の所有者が現在のユーザーと異なるか、複数のリンクがあります。所有者と保存場所を確認してください。"
        ));
    }
    Ok(())
}

pub(super) fn protect_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    mode: u32,
) -> anyhow::Result<()> {
    if metadata.file_type().is_symlink() {
        return Err(anyhow::anyhow!("設定の保存先にリンクは使用できません。"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // geteuid has no pointer arguments or failure mode.
        validate_owner(metadata, unsafe { libc::geteuid() })?;
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| {
            anyhow::anyhow!("設定のアクセス権を安全に変更できません。所有者と保存場所を確認してください: {error}")
        })?;
    }
    #[cfg(not(unix))]
    let _ = (path, metadata, mode);
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn atomic_replace(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(target_os = "windows")]
fn atomic_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("設定ファイルの親フォルダを取得できません。"))?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}
