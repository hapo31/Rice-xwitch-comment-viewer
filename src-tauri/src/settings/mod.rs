#[cfg(feature = "app")]
use crate::app_events::{emit_app_log, AppLogLevel};
use crate::launcher::{
    apply_launcher_edits, validate_launcher_resources, LauncherSettings, LauncherSettingsPatch,
};
use crate::resource_limits::{
    check_bytes, read_bounded, serialize_bounded, BoundedReadError, SizeLimitExceeded,
    MAX_SETTINGS_JSON_BYTES,
};
use crate::speech::SpeechQueueState;
use crate::twitch::TwitchAuthState;
#[cfg(feature = "app")]
use crate::twitch::{TwitchAuthStore, TwitchConnectionOwner};
use crate::SharedSettings;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "app")]
use tauri::Manager;

mod schema;
pub(crate) mod validation;
mod writer;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct AppSettings {
    pub twitch: TwitchSettings,
    pub speech: SpeechSettings,
    #[serde(default)]
    pub launcher: LauncherSettings,
    #[serde(default)]
    pub window: WindowSettings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct WindowSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<WindowPosition>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct TwitchSettings {
    pub channel_login: String,
    pub auto_connect: bool,
    #[serde(default = "default_confirm_before_stop_chat")]
    pub confirm_before_stop_chat: bool,
    #[serde(default = "default_live_chat_announcements")]
    pub live_chat_announcements: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct SpeechSettings {
    pub adapter: SpeechAdapterKind,
    #[serde(default = "default_bouyomi_host")]
    pub bouyomi_host: String,
    pub bouyomi_port: u16,
    /// Opt-in request only. Native consent is never persisted in settings.
    #[serde(default)]
    pub bouyomi_remote_mode: bool,
    #[serde(default = "default_bouyomi_speed")]
    pub bouyomi_speed: i16,
    #[serde(default = "default_bouyomi_tone")]
    pub bouyomi_tone: i16,
    #[serde(default = "default_bouyomi_volume")]
    pub bouyomi_volume: i16,
    #[serde(default = "default_bouyomi_voice")]
    pub bouyomi_voice: i16,
    pub read_user_name: bool,
    #[serde(default = "default_auto_speak")]
    pub auto_speak: bool,
    pub max_comment_length: u16,
    pub repeat_suppression_seconds: u16,
    #[serde(default)]
    pub blocked_users: Vec<String>,
    #[serde(default)]
    pub blocked_words: Vec<String>,
    #[serde(default = "default_url_handling")]
    pub url_handling: UrlHandling,
    #[serde(default = "default_read_emotes")]
    pub read_emotes: bool,
    #[serde(default = "default_connection_success_speech_enabled")]
    pub connection_success_speech_enabled: bool,
    #[serde(default)]
    pub connection_success_speech_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum SpeechAdapterKind {
    Bouyomi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum UrlHandling {
    Replace,
    Read,
    Block,
}

impl Default for UrlHandling {
    fn default() -> Self {
        default_url_handling()
    }
}

fn default_auto_speak() -> bool {
    true
}

fn default_confirm_before_stop_chat() -> bool {
    true
}

fn default_live_chat_announcements() -> bool {
    true
}

fn default_url_handling() -> UrlHandling {
    UrlHandling::Replace
}

fn default_read_emotes() -> bool {
    false
}

fn default_connection_success_speech_enabled() -> bool {
    true
}

fn default_bouyomi_speed() -> i16 {
    -1
}

fn default_bouyomi_tone() -> i16 {
    -1
}

fn default_bouyomi_volume() -> i16 {
    -1
}

fn default_bouyomi_voice() -> i16 {
    0
}

fn default_bouyomi_host() -> String {
    std::env::var("RICE_BOUYOMI_HOST")
        .ok()
        .and_then(|host| crate::speech::bouyomi::validate_bouyomi_host(&host).ok())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

pub(crate) fn default_twitch_client_id() -> String {
    option_env!("RICE_TWITCH_CLIENT_ID")
        .unwrap_or("")
        .trim()
        .to_string()
}

#[derive(Default)]
pub struct AppState {
    pub settings: SharedSettings<AppSettings>,
    /// Serializes disk transactions while keeping the published settings mutex short-lived.
    pub settings_transaction: SharedSettings<()>,
    /// Shared launcher adapters and bounded worker pool, used by every command.
    pub launcher_runtime: crate::launcher::LauncherRuntime,
    pub settings_recovery_notice: SharedSettings<Option<SettingsRecoveryNotice>>,
    #[cfg(feature = "app")]
    pub twitch_auth: std::sync::Arc<std::sync::Mutex<TwitchAuthState>>,
    #[cfg(not(feature = "app"))]
    pub twitch_auth: SharedSettings<TwitchAuthState>,
    pub speech_queue: std::sync::Arc<std::sync::Mutex<SpeechQueueState>>,
    /// Shared selection, ordering and clock for every speech operation.
    pub speech_runtime: crate::speech::runtime::SpeechRuntime,
    #[cfg(feature = "app")]
    pub twitch_connection: SharedSettings<TwitchConnectionOwner>,
    #[cfg(feature = "app")]
    pub twitch_auth_store: TwitchAuthStore,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            twitch: TwitchSettings {
                channel_login: String::new(),
                auto_connect: false,
                confirm_before_stop_chat: true,
                live_chat_announcements: true,
            },
            speech: SpeechSettings {
                adapter: SpeechAdapterKind::Bouyomi,
                bouyomi_host: default_bouyomi_host(),
                bouyomi_port: 50001,
                bouyomi_remote_mode: false,
                bouyomi_speed: -1,
                bouyomi_tone: -1,
                bouyomi_volume: -1,
                bouyomi_voice: 0,
                read_user_name: true,
                auto_speak: true,
                max_comment_length: 120,
                repeat_suppression_seconds: 2,
                blocked_users: Vec::new(),
                blocked_words: Vec::new(),
                url_handling: UrlHandling::Replace,
                read_emotes: false,
                connection_success_speech_enabled: true,
                connection_success_speech_text: String::new(),
            },
            launcher: LauncherSettings::default(),
            window: WindowSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    pub twitch: Option<TwitchSettingsPatch>,
    pub speech: Option<SpeechSettingsPatch>,
    pub launcher: Option<LauncherSettingsPatch>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TwitchSettingsPatch {
    pub channel_login: Option<String>,
    pub auto_connect: Option<bool>,
    pub confirm_before_stop_chat: Option<bool>,
    pub live_chat_announcements: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeechSettingsPatch {
    pub adapter: Option<SpeechAdapterKind>,
    pub bouyomi_host: Option<String>,
    pub bouyomi_port: Option<u16>,
    pub bouyomi_remote_mode: Option<bool>,
    pub bouyomi_speed: Option<i16>,
    pub bouyomi_tone: Option<i16>,
    pub bouyomi_volume: Option<i16>,
    pub bouyomi_voice: Option<i16>,
    pub read_user_name: Option<bool>,
    pub auto_speak: Option<bool>,
    pub max_comment_length: Option<u16>,
    pub repeat_suppression_seconds: Option<u16>,
    pub blocked_users: Option<Vec<String>>,
    pub blocked_words: Option<Vec<String>>,
    pub url_handling: Option<UrlHandling>,
    pub read_emotes: Option<bool>,
    pub connection_success_speech_enabled: Option<bool>,
    pub connection_success_speech_text: Option<String>,
}

pub struct SettingsStore;

#[derive(Debug, Clone, Serialize)]
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

fn validate_repeat_suppression_seconds(seconds: u16) -> Result<(), String> {
    if seconds <= 30 {
        Ok(())
    } else {
        Err("連投抑制秒は0から30の範囲で指定してください。".to_string())
    }
}

/// Inspect the framework-owned JSON tree before cloning any application DTO.
fn parse_settings_request(
    value: &serde_json::Value,
) -> Result<SettingsPatch, validation::ValidationError> {
    crate::resource_limits::validate_json_request(value, validation::MAX_PATCH_BYTES)
        .map_err(|message| validation::ValidationError::new("patch", "payloadTooLarge", message))?;
    if value
        .as_object()
        .is_none_or(|object| object.len() != 1 || !object.contains_key("patch"))
    {
        return Err(validation::ValidationError::new(
            "patch",
            "unknownField",
            "設定要求に未知の項目があります。入力項目を確認してください。",
        ));
    }
    let patch = value
        .get("patch")
        .ok_or_else(|| "更新する設定を指定してください。".to_string())?;
    validation::preflight_wire(patch)?;
    crate::launcher::preflight_launcher_patch(patch).map_err(|message| {
        validation::ValidationError::new("launcher.items", "invalidLauncher", message)
    })?;
    serde_json::from_value(patch.clone()).map_err(|_| {
        validation::ValidationError::new(
            "patch",
            "invalidPayload",
            "設定の項目または型が無効です。入力内容を見直してください。",
        )
    })
}

#[derive(Debug, thiserror::Error)]
enum SettingsContentError {
    #[error("設定ファイルが正しいUTF-8ではありません。")]
    Encoding(#[source] std::string::FromUtf8Error),
    #[error(transparent)]
    TooLarge(SizeLimitExceeded),
    #[error("{0}")]
    Document(String),
}

fn read_settings_text(path: &Path) -> anyhow::Result<Result<String, SettingsContentError>> {
    match read_bounded(path, MAX_SETTINGS_JSON_BYTES) {
        Ok(text) => Ok(Ok(text)),
        Err(BoundedReadError::TooLarge(error)) => Ok(Err(SettingsContentError::TooLarge(error))),
        Err(BoundedReadError::Encoding(error)) => Ok(Err(SettingsContentError::Encoding(error))),
        // IO/permission failures are not evidence of corrupt content. Fail closed.
        Err(BoundedReadError::Io(error)) => Err(error.into()),
    }
}

impl SettingsStore {
    #[cfg(feature = "app")]
    pub fn load<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<LoadedSettings> {
        let path = settings_path(app)?;
        writer::initialize(app, &path)?;
        Self::load_from_path(&path)
    }

    fn load_from_path(path: &Path) -> anyhow::Result<LoadedSettings> {
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

        let loaded = read_settings_text(path)?
            .and_then(|text| schema::decode(&text).map_err(SettingsContentError::Document));
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
            Err(reason) => Self::recover_from_invalid_primary(path, &reason.to_string()),
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

    fn save_to_path(path: &Path, settings: &AppSettings) -> anyhow::Result<()> {
        Self::save_to_path_with_fault(path, settings, SaveFault::None)
    }

    fn save_to_path_with_fault(
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

    fn save_text_to_path(path: &Path, text: &str, fault: SaveFault) -> anyhow::Result<()> {
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
                    Err(reason) => SettingsContentError::Document(reason),
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
enum SaveFault {
    None,
    TempWrite,
    Replace,
}

fn backup_path(path: &Path) -> PathBuf {
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

fn write_temp_file(path: &Path, contents: &[u8], fault: SaveFault) -> anyhow::Result<PathBuf> {
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
fn protect_storage(path: &Path) -> anyhow::Result<()> {
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

fn protect_existing_file(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(anyhow::anyhow!("設定ファイルにリンクや通常以外の種類は使用できません。設定の保存場所を確認してください。"));
            }
            protect_metadata(path, &metadata, 0o600)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(unix)]
fn validate_owner(metadata: &fs::Metadata, expected: u32) -> anyhow::Result<()> {
    use std::os::unix::fs::MetadataExt;
    if metadata.uid() != expected || (metadata.is_file() && metadata.nlink() != 1) {
        return Err(anyhow::anyhow!("設定の所有者が現在のユーザーと異なるか、複数のリンクがあります。所有者と保存場所を確認してください。"));
    }
    Ok(())
}

fn protect_metadata(path: &Path, metadata: &fs::Metadata, mode: u32) -> anyhow::Result<()> {
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

#[cfg(feature = "app")]
#[tauri::command]
pub fn settings_get(state: tauri::State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state
        .settings
        .lock()
        .map_err(|error| error.to_string())?
        .clone())
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn settings_take_recovery_notice(
    state: tauri::State<'_, AppState>,
) -> Result<Option<SettingsRecoveryNotice>, String> {
    state
        .settings_recovery_notice
        .lock()
        .map_err(|error| error.to_string())
        .map(|mut notice| notice.take())
}

#[cfg(test)]
pub(crate) fn update_settings_transaction(
    settings: &mut AppSettings,
    update: impl FnOnce(&mut AppSettings) -> Result<(), String>,
    save: impl FnOnce(&AppSettings) -> Result<(), String>,
) -> Result<(), String> {
    update_settings_transaction_with_error(settings, update, save)
}

pub(crate) fn update_shared_settings_transaction<E: From<String>>(
    settings: &SharedSettings<AppSettings>,
    transaction: &SharedSettings<()>,
    update: impl FnOnce(&mut AppSettings) -> Result<(), E>,
    save: impl FnOnce(&AppSettings) -> Result<(), E>,
) -> Result<(AppSettings, AppSettings), E> {
    let _transaction = transaction
        .lock()
        .map_err(|error| E::from(error.to_string()))?;
    let previous = settings
        .lock()
        .map_err(|error| E::from(error.to_string()))?
        .clone();
    let mut candidate = previous.clone();
    apply_settings_candidate(&mut candidate, update)?;
    save(&candidate)?;
    *settings
        .lock()
        .map_err(|error| E::from(error.to_string()))? = candidate.clone();
    Ok((previous, candidate))
}

fn update_settings_transaction_with_error<E: From<String>>(
    settings: &mut AppSettings,
    update: impl FnOnce(&mut AppSettings) -> Result<(), E>,
    save: impl FnOnce(&AppSettings) -> Result<(), E>,
) -> Result<(), E> {
    let mut candidate = settings.clone();
    apply_settings_candidate(&mut candidate, update)?;
    save(&candidate)?;
    *settings = candidate;
    Ok(())
}

fn apply_settings_candidate<E: From<String>>(
    candidate: &mut AppSettings,
    update: impl FnOnce(&mut AppSettings) -> Result<(), E>,
) -> Result<(), E> {
    update(candidate)?;
    validate_launcher_resources(&candidate.launcher.items).map_err(E::from)
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn settings_update(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<AppSettings, validation::ValidationError> {
    let patch = parse_settings_request(crate::resource_limits::request_json(&request)?)?;
    validation::validate_patch(&patch)?;
    let (previous, settings) = update_shared_settings_transaction(
        &state.settings,
        &state.settings_transaction,
        |candidate| {
            apply_patch(candidate, patch)?;
            validation::validate_settings(candidate)
        },
        |candidate| {
            SettingsStore::save(&app, candidate).map_err(|error| {
                if error.is::<schema::ReadOnlySettings>() {
                    return validation::ValidationError::new(
                        "settings",
                        "unsupportedSchema",
                        schema::READ_ONLY_MESSAGE,
                    );
                }
                validation::ValidationError::new(
                    "settings",
                    "persistenceFailed",
                    "設定を保存できませんでした。保存先の空き容量・権限を確認してください。",
                )
            })
        },
    )?;
    if (
        previous.speech.bouyomi_host,
        previous.speech.bouyomi_port,
        previous.speech.bouyomi_remote_mode,
    ) != (
        settings.speech.bouyomi_host.clone(),
        settings.speech.bouyomi_port,
        settings.speech.bouyomi_remote_mode,
    ) {
        state.speech_runtime.destination_policy().revoke();
    }
    emit_app_log(&app, AppLogLevel::Info, "設定を保存しました。");
    Ok(settings)
}

fn apply_validated_settings_patch(
    settings: &mut AppSettings,
    patch: SettingsPatch,
    save: impl FnOnce(&AppSettings) -> Result<(), validation::ValidationError>,
) -> Result<(), validation::ValidationError> {
    validation::validate_patch(&patch)?;
    update_settings_transaction_with_error(
        settings,
        |candidate| {
            apply_patch(candidate, patch)?;
            validation::validate_settings(candidate)
        },
        save,
    )
}

fn apply_patch(settings: &mut AppSettings, patch: SettingsPatch) -> Result<(), String> {
    if let Some(twitch) = patch.twitch {
        if let Some(channel_login) = twitch.channel_login {
            settings.twitch.channel_login = validation::TwitchLogin::parse(&channel_login, true)
                .map_err(|error| error.to_string())?
                .into_string();
        }
        if let Some(auto_connect) = twitch.auto_connect {
            settings.twitch.auto_connect = auto_connect;
        }
        if let Some(confirm_before_stop_chat) = twitch.confirm_before_stop_chat {
            settings.twitch.confirm_before_stop_chat = confirm_before_stop_chat;
        }
        if let Some(live_chat_announcements) = twitch.live_chat_announcements {
            settings.twitch.live_chat_announcements = live_chat_announcements;
        }
    }

    if let Some(speech) = patch.speech {
        if let Some(mode) = speech.bouyomi_remote_mode {
            settings.speech.bouyomi_remote_mode = mode;
        }
        if let Some(adapter) = speech.adapter {
            settings.speech.adapter = adapter;
        }
        if let Some(host) = speech.bouyomi_host {
            settings.speech.bouyomi_host = crate::speech::bouyomi::validate_bouyomi_host(&host)?;
        }
        if let Some(port) = speech.bouyomi_port {
            if port == 0 {
                return Err("棒読みちゃんのポート番号が無効です。".to_string());
            }
            settings.speech.bouyomi_port = port;
        }
        if let Some(speed) = speech.bouyomi_speed {
            settings.speech.bouyomi_speed = validate_range(speed, -1, 300, "速度")?;
        }
        if let Some(tone) = speech.bouyomi_tone {
            settings.speech.bouyomi_tone = validate_range(tone, -1, 200, "音程")?;
        }
        if let Some(volume) = speech.bouyomi_volume {
            settings.speech.bouyomi_volume = validate_range(volume, -1, 100, "音量")?;
        }
        if let Some(voice) = speech.bouyomi_voice {
            settings.speech.bouyomi_voice = validate_range(voice, 0, 30000, "声質")?;
        }
        if let Some(read_user_name) = speech.read_user_name {
            settings.speech.read_user_name = read_user_name;
        }
        if let Some(auto_speak) = speech.auto_speak {
            settings.speech.auto_speak = auto_speak;
        }
        if let Some(max_length) = speech.max_comment_length {
            settings.speech.max_comment_length = max_length.clamp(1, 500);
        }
        if let Some(seconds) = speech.repeat_suppression_seconds {
            validate_repeat_suppression_seconds(seconds)?;
            settings.speech.repeat_suppression_seconds = seconds;
        }
        if let Some(blocked_users) = speech.blocked_users {
            settings.speech.blocked_users = normalize_rule_list(blocked_users, RuleListKind::User)?;
        }
        if let Some(blocked_words) = speech.blocked_words {
            settings.speech.blocked_words = normalize_rule_list(blocked_words, RuleListKind::Word)?;
        }
        if let Some(url_handling) = speech.url_handling {
            settings.speech.url_handling = url_handling;
        }
        if let Some(read_emotes) = speech.read_emotes {
            settings.speech.read_emotes = read_emotes;
        }
        if let Some(enabled) = speech.connection_success_speech_enabled {
            settings.speech.connection_success_speech_enabled = enabled;
        }
        if let Some(text) = speech.connection_success_speech_text {
            settings.speech.connection_success_speech_text =
                text.trim().chars().take(120).collect();
        }
    }

    if let Some(launcher) = patch.launcher {
        if let Some(items) = launcher.items {
            settings.launcher.items = apply_launcher_edits(&settings.launcher.items, items)?;
        }
    }

    Ok(())
}

const RULE_LIST_LIMIT: usize = 200;

#[derive(Clone, Copy)]
enum RuleListKind {
    User,
    Word,
}

fn normalize_rule_list(items: Vec<String>, kind: RuleListKind) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for item in items {
        let item = item.trim();
        let item = match kind {
            RuleListKind::User => item.trim_start_matches('@'),
            RuleListKind::Word => item,
        };
        if item.is_empty() {
            continue;
        }

        if seen.insert(item.to_ascii_lowercase()) {
            normalized.push(item.to_string());
        }
    }

    if normalized.len() > RULE_LIST_LIMIT {
        return Err(format!(
            "NG {}は最大 {RULE_LIST_LIMIT} 件です。{} 件超過しているため保存できません。",
            match kind {
                RuleListKind::User => "ユーザー",
                RuleListKind::Word => "ワード",
            },
            normalized.len() - RULE_LIST_LIMIT
        ));
    }

    Ok(normalized)
}

fn validate_range(value: i16, min: i16, max: i16, label: &str) -> Result<i16, String> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(format!("棒読みちゃんの{label}が無効です。"))
    }
}

#[cfg(feature = "app")]
fn settings_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> anyhow::Result<std::path::PathBuf> {
    Ok(app.path().app_data_dir()?.join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::{
        apply_patch, backup_path, update_settings_transaction, AppSettings, SaveFault,
        SettingsPatch, SettingsStore, WindowPosition,
    };
    use crate::launcher::{normalize_launcher_items, LauncherItem, LauncherItemKind};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn slow_settings_transaction_does_not_hold_the_published_settings_lock() {
        let settings = std::sync::Arc::new(std::sync::Mutex::new(AppSettings::default()));
        let transaction = std::sync::Arc::new(std::sync::Mutex::new(()));
        let (entered_sender, entered_receiver) = std::sync::mpsc::channel();
        let (continue_sender, continue_receiver) = std::sync::mpsc::channel();
        let worker_settings = settings.clone();
        let worker_transaction = transaction.clone();
        let worker = std::thread::spawn(move || {
            super::update_shared_settings_transaction(
                &worker_settings,
                &worker_transaction,
                |candidate| {
                    entered_sender.send(()).unwrap();
                    continue_receiver.recv().unwrap();
                    candidate.twitch.channel_login = "next-channel".into();
                    Ok::<(), String>(())
                },
                |_| Ok::<(), String>(()),
            )
        });

        entered_receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("transaction reached slow candidate work");
        let visible = settings
            .try_lock()
            .expect("published settings lock is available during candidate work");
        assert!(visible.twitch.channel_login.is_empty());
        drop(visible);

        continue_sender.send(()).unwrap();
        let (previous, current) = worker.join().unwrap().unwrap();
        assert!(previous.twitch.channel_login.is_empty());
        assert_eq!(current.twitch.channel_login, "next-channel");
        assert_eq!(
            settings.lock().unwrap().twitch.channel_login,
            "next-channel"
        );
    }

    #[test]
    fn concurrent_settings_transactions_publish_in_order_without_lost_updates() {
        let settings = std::sync::Arc::new(std::sync::Mutex::new(AppSettings::default()));
        let transaction = std::sync::Arc::new(std::sync::Mutex::new(()));
        let (first_entered_sender, first_entered_receiver) = std::sync::mpsc::channel();
        let (continue_first_sender, continue_first_receiver) = std::sync::mpsc::channel();
        let first_settings = settings.clone();
        let first_transaction = transaction.clone();
        let first = std::thread::spawn(move || {
            super::update_shared_settings_transaction(
                &first_settings,
                &first_transaction,
                |candidate| {
                    first_entered_sender.send(()).unwrap();
                    continue_first_receiver.recv().unwrap();
                    candidate.twitch.channel_login = "first".into();
                    Ok::<(), String>(())
                },
                |_| Ok::<(), String>(()),
            )
        });
        first_entered_receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("first transaction entered candidate work");

        let (second_entered_sender, second_entered_receiver) = std::sync::mpsc::channel();
        let second_settings = settings.clone();
        let second_transaction = transaction.clone();
        let second = std::thread::spawn(move || {
            super::update_shared_settings_transaction(
                &second_settings,
                &second_transaction,
                |candidate| {
                    second_entered_sender.send(()).unwrap();
                    candidate.speech.bouyomi_port = 50_002;
                    Ok::<(), String>(())
                },
                |_| Ok::<(), String>(()),
            )
        });
        assert!(second_entered_receiver
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err());

        continue_first_sender.send(()).unwrap();
        first.join().unwrap().unwrap();
        second_entered_receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("second transaction starts after the first commits");
        second.join().unwrap().unwrap();

        let settings = settings.lock().unwrap();
        assert_eq!(settings.twitch.channel_login, "first");
        assert_eq!(settings.speech.bouyomi_port, 50_002);
    }

    #[test]
    fn failed_shared_settings_persistence_does_not_publish_candidate_state() {
        let settings = std::sync::Mutex::new(AppSettings::default());
        let transaction = std::sync::Mutex::new(());
        let before = settings.lock().unwrap().clone();
        let result = super::update_shared_settings_transaction(
            &settings,
            &transaction,
            |candidate| {
                candidate.twitch.channel_login = "unsaved".into();
                Ok::<(), String>(())
            },
            |_| Err::<(), String>("fixture persistence failure".into()),
        );

        assert_eq!(result.unwrap_err(), "fixture persistence failure");
        assert_eq!(
            settings.lock().unwrap().twitch.channel_login,
            before.twitch.channel_login
        );
    }

    #[test]
    fn maximum_launcher_roundtrip_stays_within_time_and_rust_heap_budget() {
        const TEST: &str =
            "settings::tests::maximum_launcher_roundtrip_stays_within_time_and_rust_heap_budget";
        if std::env::var_os("RICE_LAUNCHER_BUDGET_CHILD").is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env("RICE_LAUNCHER_BUDGET_CHILD", "1")
                .status()
                .unwrap();
            assert!(status.success(), "isolated maximum-payload budget test");
            return;
        }
        for json_bytes in [None, Some(super::MAX_SETTINGS_JSON_BYTES)] {
            let path = settings_path_for_test("launcher-budget");
            let mut settings = crate::launcher::bounds_tests::full_quota_settings();
            SettingsStore::save_to_path(&path, &settings).unwrap();
            let pad_json = |path: &std::path::Path| {
                if let Some(json_bytes) = json_bytes {
                    let mut padding = json_bytes - fs::metadata(path).unwrap().len() as usize;
                    let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
                    while padding > 0 {
                        let count = padding.min(4096);
                        std::io::Write::write_all(&mut file, &vec![b' '; count]).unwrap();
                        padding -= count;
                    }
                    file.sync_all().unwrap();
                }
            };
            // A maximum-size valid wire document uses trailing JSON whitespace,
            // not an oversized domain field that the validator would reject.
            pad_json(&path);
            let baseline = crate::resource_limits::allocation::start();
            let start = std::time::Instant::now();
            update_settings_transaction(
                &mut settings,
                |candidate| {
                    candidate.launcher.items[0].order = 201;
                    Ok(())
                },
                |candidate| {
                    SettingsStore::save_to_path(&path, candidate).map_err(|error| error.to_string())
                },
            )
            .unwrap();
            pad_json(&path);
            let loaded = SettingsStore::load_from_path(&path).unwrap();
            assert_eq!(loaded.settings.launcher.items, settings.launcher.items);
            assert_eq!(loaded.settings.launcher.items.len(), 200);
            let elapsed = start.elapsed();
            let peak = crate::resource_limits::allocation::peak_delta(baseline);
            println!("Launcher budget: items=200 icons=4194304 JSON={} elapsed_ms={} incremental_rust_heap={peak}", fs::metadata(&path).unwrap().len(), elapsed.as_millis());
            assert!(
                elapsed < std::time::Duration::from_secs(5),
                "maximum transaction/backup/load budget: {elapsed:?}"
            );
            let budget_mib = if json_bytes.is_some() { 40 } else { 32 };
            assert!(
            peak <= budget_mib * 1024 * 1024,
            "Rust-owned incremental live heap, including conservative realloc coexistence: {peak}"
        );
            cleanup(&path);
        }
    }

    #[test]
    fn bounded_request_rejects_backend_fields_new_ids_and_large_json() {
        use super::parse_settings_request;
        for field in ["target", "kind", "iconDataUrl"] {
            let mut edit =
                serde_json::json!({ "id": "existing", "displayName": "name", "order": 0 });
            edit[field] = serde_json::json!("injected");
            assert!(
                parse_settings_request(
                    &serde_json::json!({"patch": {"launcher": {"items": [edit]}}})
                )
                .is_err(),
                "{field}"
            );
        }
        let patch = parse_settings_request(&serde_json::json!({"patch": {"twitch": {"channelLogin": "candidate"}, "launcher": {"items": [{"id": "new", "displayName": "name", "order": 0}]}}})).unwrap();
        let mut settings = AppSettings::default();
        let saved = std::cell::Cell::new(false);
        assert!(update_settings_transaction(
            &mut settings,
            |candidate| apply_patch(candidate, patch),
            |_| {
                saved.set(true);
                Ok(())
            }
        )
        .is_err());
        assert_eq!(settings.twitch.channel_login, "");
        assert!(!saved.get());
        let huge = serde_json::json!({"patch": {"speech": {"blockedWords": ["x".repeat(super::MAX_SETTINGS_JSON_BYTES)]}}});
        assert!(parse_settings_request(&huge)
            .unwrap_err()
            .message
            .contains("最大"));
    }

    #[test]
    fn invalid_wire_and_domain_patch_never_save_publish_or_change_either_file() {
        let path = settings_path_for_test("domain-validation");
        let mut settings = AppSettings::default();
        SettingsStore::save_to_path(&path, &settings).unwrap();
        SettingsStore::save_to_path(&path, &settings).unwrap();
        let previous = fs::read(&path).unwrap();
        let previous_backup = fs::read(backup_path(&path)).unwrap();
        let cases = [
            serde_json::json!({"twitch":{"channelLogin":"ab"}}),
            serde_json::json!({"speech":{"bouyomiHost":""}}),
            serde_json::json!({"speech":{"bouyomiPort":0}}),
            serde_json::json!({"speech":{"maxCommentLength":0}}),
            serde_json::json!({"speech":{"bouyomiSpeed":301}}),
            serde_json::json!({"speech":{"repeatSuppressionSeconds":31}}),
            serde_json::json!({"speech":{"connectionSuccessSpeechText":"😀".repeat(121)}}),
            serde_json::json!({"speech":{"blockedWords":["x".repeat(501)]}}),
            serde_json::json!({"speech":{"blockedUsers":["invalid-login"]}}),
            serde_json::json!({"speech":{"autoSpeek":true}}),
            serde_json::json!({"twitch":{"channelLogn":"abc"}}),
            serde_json::json!({"speeech":{}}),
        ];
        for patch in cases {
            let saved = std::cell::Cell::new(false);
            let result = super::parse_settings_request(&serde_json::json!({"patch":patch}))
                .and_then(|patch| {
                    super::apply_validated_settings_patch(&mut settings, patch, |_| {
                        saved.set(true);
                        Ok(())
                    })
                });
            let error = result.unwrap_err();
            assert!(!error.field.is_empty() && !error.code.is_empty());
            assert!(!error.recovery.is_empty());
            assert!(!saved.get());
            assert_eq!(fs::read(&path).unwrap(), previous);
            assert_eq!(fs::read(backup_path(&path)).unwrap(), previous_backup);
            assert_eq!(
                serde_json::to_value(&settings).unwrap(),
                serde_json::to_value(AppSettings::default()).unwrap()
            );
        }
        cleanup(&path);
    }

    #[tokio::test]
    async fn manually_edited_settings_cannot_persist_or_forge_remote_consent() {
        let path = settings_path_for_test("remote-consent-tampering");
        SettingsStore::save_to_path(&path, &AppSettings::default()).unwrap();
        for remote_mode in [false, true] {
            let mut wire = serde_json::to_value(AppSettings::default()).unwrap();
            wire["speech"]["bouyomiHost"] = serde_json::json!("10.0.0.1");
            wire["speech"]["bouyomiRemoteMode"] = serde_json::json!(remote_mode);
            // These attacker-controlled file values are not an approval token.
            wire["speech"]["remoteConsent"] = serde_json::json!(true);
            wire["speech"]["approvedEndpoint"] = serde_json::json!("10.0.0.1:50001");
            fs::write(&path, serde_json::to_vec_pretty(&wire).unwrap()).unwrap();
            let edited_bytes = fs::read(&path).unwrap();
            let loaded = SettingsStore::load_from_path(&path).unwrap();
            assert_eq!(loaded.settings.speech.bouyomi_host, "10.0.0.1");
            assert_eq!(loaded.settings.speech.bouyomi_remote_mode, remote_mode);
            assert!(loaded.recovery_notice.is_none());
            // Fresh production runtimes model restarts. The literal address
            // needs no DNS and is rejected before any real network operation.
            for _ in 0..2 {
                let runtime = crate::speech::runtime::SpeechRuntime::default();
                let selected = runtime.select(&loaded.settings.speech).unwrap();
                let failure = selected.lock().await.health_check().await.unwrap_err();
                assert_eq!(failure.code, crate::speech::FailureCode::Configuration);
                assert!(!failure.retryable);
                assert!(failure.user_message.contains("外部へは送信していません"));
            }
            assert_eq!(fs::read(&path).unwrap(), edited_bytes);
        }
        cleanup(&path);
    }

    #[test]
    fn over_quota_and_large_serialization_leave_memory_primary_backup_and_temps_unchanged() {
        let path = settings_path_for_test("bounded-transaction");
        let mut settings = settings_with_channel("original");
        SettingsStore::save_to_path(&path, &settings).unwrap();
        SettingsStore::save_to_path(&path, &settings).unwrap();
        let primary = fs::read(&path).unwrap();
        let backup = fs::read(backup_path(&path)).unwrap();
        let items = crate::launcher::bounds_tests::over_quota_icon_items();
        for launcher_failure in [true, false] {
            let result = update_settings_transaction(
                &mut settings,
                |candidate| {
                    candidate.twitch.channel_login = "candidate".into();
                    if launcher_failure {
                        candidate.launcher.items = items.clone();
                    } else {
                        candidate.speech.blocked_words =
                            vec!["x".repeat(super::MAX_SETTINGS_JSON_BYTES)];
                    }
                    Ok(())
                },
                |candidate| {
                    SettingsStore::save_to_path(&path, candidate).map_err(|error| error.to_string())
                },
            );
            assert!(result.is_err());
            assert_eq!(settings.twitch.channel_login, "original");
            assert!(settings.launcher.items.is_empty());
            assert_eq!(fs::read(&path).unwrap(), primary);
            assert_eq!(fs::read(backup_path(&path)).unwrap(), backup);
            assert_eq!(path.parent().unwrap().read_dir().unwrap().count(), 2);
        }
        let huge = " ".repeat(super::MAX_SETTINGS_JSON_BYTES + 1);
        assert!(SettingsStore::save_text_to_path(&path, &huge, SaveFault::None).is_err());
        assert_eq!(fs::read(&path).unwrap(), primary);
        assert_eq!(fs::read(backup_path(&path)).unwrap(), backup);
        cleanup(&path);
    }

    #[test]
    fn oversized_primary_and_backup_are_quarantined_without_full_read() {
        for bad_backup in [false, true] {
            let path = settings_path_for_test("bounded-recovery");
            let settings = settings_with_channel("valid_backup");
            SettingsStore::save_to_path(&path, &settings).unwrap();
            SettingsStore::save_to_path(&path, &settings).unwrap();
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(super::MAX_SETTINGS_JSON_BYTES as u64 + 1)
                .unwrap();
            if bad_backup {
                std::fs::File::options()
                    .write(true)
                    .open(backup_path(&path))
                    .unwrap()
                    .set_len(super::MAX_SETTINGS_JSON_BYTES as u64 + 1)
                    .unwrap();
            }
            let loaded = SettingsStore::load_from_path(&path).unwrap();
            assert_eq!(
                loaded.settings.twitch.channel_login,
                if bad_backup { "" } else { "valid_backup" }
            );
            assert!(loaded.recovery_notice.unwrap().message.contains("最大"));
            let quarantined: Vec<_> = path
                .parent()
                .unwrap()
                .read_dir()
                .unwrap()
                .map(Result::unwrap)
                .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
                .collect();
            assert_eq!(quarantined.len(), if bad_backup { 2 } else { 1 });
            assert!(quarantined
                .iter()
                .all(|entry| entry.metadata().unwrap().len()
                    == super::MAX_SETTINGS_JSON_BYTES as u64 + 1));
            cleanup(&path);
        }
    }

    pub(super) fn settings_path_for_test(name: &str) -> PathBuf {
        let counter = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "rice-settings-{name}-{}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create test directory");
        directory.join("settings.json")
    }

    fn settings_with_channel(channel_login: &str) -> AppSettings {
        let mut settings = AppSettings::default();
        settings.twitch.channel_login = channel_login.to_string();
        settings
    }

    pub(super) fn cleanup(path: &std::path::Path) {
        fs::remove_dir_all(path.parent().expect("test path parent"))
            .expect("remove test directory");
    }

    #[cfg(unix)]
    #[test]
    fn settings_permissions_are_owner_only_under_both_umasks() {
        use std::os::unix::fs::PermissionsExt;
        if std::env::var_os("RICE_PERMISSION_TEST_CHILD").is_none() {
            for mask in ["022", "000"] {
                let status = std::process::Command::new("sh")
                    .args(["-c", "umask \"$1\"; exec \"$2\" --exact settings::tests::settings_permissions_are_owner_only_under_both_umasks", "rice-permissions", mask])
                    .arg(std::env::current_exe().unwrap())
                    .env("RICE_PERMISSION_TEST_CHILD", "1")
                    .status()
                    .expect("run isolated umask test");
                assert!(status.success(), "umask {mask}");
            }
            return;
        }
        let path = settings_path_for_test("private-mode");
        fs::remove_dir(path.parent().unwrap()).unwrap();
        SettingsStore::save_to_path(&path, &AppSettings::default()).unwrap();
        let mode =
            |path: &std::path::Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(path.parent().unwrap()), 0o700);
        assert_eq!(mode(&path), 0o600);
        let temporary = super::write_temp_file(&path, b"private", SaveFault::None).unwrap();
        assert_eq!(mode(&temporary), 0o600);
        fs::remove_file(temporary).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
        fs::set_permissions(path.parent().unwrap(), fs::Permissions::from_mode(0o777)).unwrap();
        SettingsStore::load_from_path(&path).unwrap();
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
        SettingsStore::save_to_path(&path, &settings_with_channel("updated")).unwrap();
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(&backup_path(&path)), 0o600);
        fs::write(&path, "broken").unwrap();
        SettingsStore::load_from_path(&path).unwrap();
        for entry in fs::read_dir(path.parent().unwrap()).unwrap() {
            assert_eq!(mode(&entry.unwrap().path()), 0o600);
        }
        cleanup(&path);
    }

    #[cfg(unix)]
    #[test]
    fn settings_reject_symlinks_non_regular_files_and_foreign_owners() {
        use std::os::unix::fs::{symlink, MetadataExt};
        let path = settings_path_for_test("unsafe-path");
        let outside = path.parent().unwrap().join("outside.json");
        fs::write(&outside, "private").unwrap();
        symlink(&outside, &path).unwrap();
        assert!(SettingsStore::load_from_path(&path).is_err());
        assert!(SettingsStore::save_to_path(&path, &AppSettings::default()).is_err());
        assert_eq!(fs::read_to_string(&outside).unwrap(), "private");
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(SettingsStore::load_from_path(&path).is_err());
        fs::remove_dir(&path).unwrap();
        let metadata = fs::metadata(&outside).unwrap();
        assert!(super::validate_owner(&metadata, metadata.uid().wrapping_add(1)).is_err());
        let linked = path.parent().unwrap().join("linked.json");
        fs::hard_link(&outside, &linked).unwrap();
        assert!(super::protect_existing_file(&linked).is_err());
        let directory_link = path.parent().unwrap().join("directory-link");
        symlink(path.parent().unwrap(), &directory_link).unwrap();
        assert!(SettingsStore::load_from_path(&directory_link.join("settings.json")).is_err());
        cleanup(&path);
    }

    #[test]
    fn legacy_settings_without_launcher_use_an_empty_default() {
        let settings = AppSettings::default();
        let mut value = serde_json::to_value(settings).expect("serialize default settings");
        value
            .as_object_mut()
            .expect("settings must be an object")
            .remove("launcher");

        let restored: AppSettings =
            serde_json::from_value(value).expect("deserialize legacy settings");

        assert!(restored.launcher.items.is_empty());
    }

    #[test]
    fn legacy_settings_without_window_position_keep_the_default() {
        let settings = AppSettings::default();
        let mut value = serde_json::to_value(settings).expect("serialize default settings");
        value
            .as_object_mut()
            .expect("settings must be an object")
            .remove("window");

        let restored: AppSettings =
            serde_json::from_value(value).expect("deserialize legacy settings");

        assert_eq!(restored.window.position, None);
    }

    #[test]
    fn window_position_round_trips_through_settings_json() {
        let mut settings = AppSettings::default();
        settings.window.position = Some(WindowPosition { x: -1280, y: 96 });

        let restored: AppSettings =
            serde_json::from_str(&serde_json::to_string(&settings).expect("serialize settings"))
                .expect("deserialize settings");

        assert_eq!(
            restored.window.position,
            Some(WindowPosition { x: -1280, y: 96 })
        );
    }

    #[test]
    fn legacy_settings_without_live_chat_announcements_enable_it_by_default() {
        let settings = AppSettings::default();
        let mut value = serde_json::to_value(settings).expect("serialize default settings");
        value["twitch"]
            .as_object_mut()
            .expect("twitch settings must be an object")
            .remove("liveChatAnnouncements");

        let restored: AppSettings =
            serde_json::from_value(value).expect("deserialize legacy settings");

        assert!(restored.twitch.live_chat_announcements);
    }

    #[test]
    fn live_chat_announcement_setting_can_be_disabled() {
        let mut settings = AppSettings::default();
        let patch: SettingsPatch = serde_json::from_value(serde_json::json!({
            "twitch": { "liveChatAnnouncements": false }
        }))
        .expect("deserialize patch");

        apply_patch(&mut settings, patch).expect("apply patch");

        assert!(!settings.twitch.live_chat_announcements);
    }

    #[test]
    fn repeat_suppression_boundaries_are_preserved_by_settings_patch() {
        for seconds in [0, 1, 2, 30] {
            let mut settings = AppSettings::default();
            let patch: SettingsPatch = serde_json::from_value(serde_json::json!({
                "speech": { "repeatSuppressionSeconds": seconds }
            }))
            .expect("deserialize patch");

            apply_patch(&mut settings, patch).expect("apply patch");

            assert_eq!(settings.speech.repeat_suppression_seconds, seconds);
        }
    }

    #[test]
    fn repeat_suppression_outside_the_frontend_range_is_rejected() {
        let mut settings = AppSettings::default();
        let patch: SettingsPatch = serde_json::from_value(serde_json::json!({
            "speech": { "repeatSuppressionSeconds": 31 }
        }))
        .expect("deserialize patch");

        assert_eq!(
            apply_patch(&mut settings, patch),
            Err("連投抑制秒は0から30の範囲で指定してください。".to_string())
        );
        assert_eq!(settings.speech.repeat_suppression_seconds, 2);
    }

    #[test]
    fn repeat_suppression_boundaries_are_preserved_when_loading_settings() {
        for seconds in [0, 1, 2, 30] {
            let path = settings_path_for_test(&format!("load-repeat-{seconds}"));
            let mut settings = AppSettings::default();
            settings.speech.repeat_suppression_seconds = seconds;
            fs::write(
                &path,
                serde_json::to_string(&settings).expect("serialize settings"),
            )
            .expect("write settings");

            let loaded = SettingsStore::load_from_path(&path).expect("load valid settings");

            assert_eq!(loaded.settings.speech.repeat_suppression_seconds, seconds);
            assert!(loaded.recovery_notice.is_none());
            cleanup(&path);
        }
    }

    #[test]
    fn invalid_repeat_suppression_defaults_only_that_field_without_a_notice() {
        let path = settings_path_for_test("recover-invalid-repeat");
        let mut invalid = AppSettings::default();
        invalid.speech.repeat_suppression_seconds = 31;
        let mut backup = AppSettings::default();
        backup.speech.repeat_suppression_seconds = 1;
        fs::write(
            &path,
            serde_json::to_string(&invalid).expect("serialize invalid settings"),
        )
        .expect("write invalid settings");
        fs::write(
            backup_path(&path),
            serde_json::to_string(&backup).expect("serialize backup"),
        )
        .expect("write backup");

        let loaded = SettingsStore::load_from_path(&path).expect("recover from backup");

        assert_eq!(loaded.settings.speech.repeat_suppression_seconds, 2);
        assert!(loaded.recovery_notice.is_none());
        let original: AppSettings =
            serde_json::from_str(&fs::read_to_string(backup_path(&path)).unwrap()).unwrap();
        assert_eq!(original.speech.repeat_suppression_seconds, 31);
        cleanup(&path);
    }

    #[test]
    fn save_keeps_a_complete_backup_and_replaces_the_primary() {
        let path = settings_path_for_test("atomic-save");
        let previous = settings_with_channel("previous");
        let next = settings_with_channel("next");
        SettingsStore::save_to_path(&path, &previous).expect("save previous settings");

        SettingsStore::save_to_path(&path, &next).expect("atomically save next settings");

        let primary: AppSettings =
            serde_json::from_str(&fs::read_to_string(&path).expect("read primary"))
                .expect("primary must be complete JSON");
        let backup: AppSettings =
            serde_json::from_str(&fs::read_to_string(backup_path(&path)).expect("read backup"))
                .expect("backup must be complete JSON");
        assert_eq!(primary.twitch.channel_login, "next");
        assert_eq!(backup.twitch.channel_login, "previous");
        cleanup(&path);
    }

    #[test]
    fn disk_full_while_writing_the_temp_file_preserves_the_primary() {
        let path = settings_path_for_test("disk-full");
        let previous = settings_with_channel("previous");
        SettingsStore::save_to_path(&path, &previous).expect("save previous settings");
        let original = fs::read_to_string(&path).expect("read primary");

        let result = SettingsStore::save_text_to_path(&path, "{}", SaveFault::TempWrite);

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).expect("read primary"), original);
        cleanup(&path);
    }

    #[test]
    fn replace_failure_preserves_the_primary_and_leaves_a_valid_backup() {
        let path = settings_path_for_test("replace-failure");
        let previous = settings_with_channel("previous");
        SettingsStore::save_to_path(&path, &previous).expect("save previous settings");
        let original = fs::read_to_string(&path).expect("read primary");

        let result = SettingsStore::save_text_to_path(&path, "{}", SaveFault::Replace);

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).expect("read primary"), original);
        let backup: AppSettings =
            serde_json::from_str(&fs::read_to_string(backup_path(&path)).expect("read backup"))
                .expect("backup must be complete JSON");
        assert_eq!(backup.twitch.channel_login, "previous");
        cleanup(&path);
    }

    #[test]
    fn invalid_utf8_primary_recovers_backup_and_preserves_original_bytes() {
        let path = settings_path_for_test("utf8-backup");
        let invalid = b"{\"private\":\"\xff\xfe\"}";
        fs::write(&path, invalid).unwrap();
        let backup = serde_json::to_vec(&super::schema::PersistedSettings::new(
            &settings_with_channel("recovered"),
        ))
        .unwrap();
        fs::write(backup_path(&path), &backup).unwrap();
        assert!(matches!(
            super::read_settings_text(&path).unwrap(),
            Err(super::SettingsContentError::Encoding(_))
        ));
        let loaded = SettingsStore::load_from_path(&path).unwrap();
        assert_eq!(loaded.settings.twitch.channel_login, "recovered");
        let notice = loaded.recovery_notice.unwrap().message;
        assert!(notice.contains("UTF-8") && notice.contains("バックアップから復旧"));
        assert!(!notice.contains("private"));
        assert_eq!(fs::read(&path).unwrap(), backup);
        assert_eq!(quarantined_bytes(&path), vec![invalid.to_vec()]);
        cleanup(&path);
    }

    fn quarantined_bytes(path: &std::path::Path) -> Vec<Vec<u8>> {
        let mut bytes: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(Result::unwrap)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
            .map(|entry| fs::read(entry.path()).unwrap())
            .collect();
        bytes.sort();
        bytes
    }

    #[test]
    fn invalid_utf8_backup_and_broken_primary_recover_defaults_and_preserve_both() {
        for primary in [b"{\"twitch\":".as_slice(), b"\xff\x80".as_slice()] {
            let path = settings_path_for_test("utf8-defaults");
            let backup = b"\xfe\xff";
            fs::write(&path, primary).unwrap();
            fs::write(backup_path(&path), backup).unwrap();
            let loaded = SettingsStore::load_from_path(&path).unwrap();
            assert_eq!(loaded.settings.twitch.channel_login, "");
            let notice = loaded.recovery_notice.unwrap().message;
            assert!(notice.contains("UTF-8") && notice.contains("既定値"));
            let mut originals = vec![primary.to_vec(), backup.to_vec()];
            originals.sort();
            assert_eq!(quarantined_bytes(&path), originals);
            super::schema::decode(&fs::read_to_string(&path).unwrap()).unwrap();
            cleanup(&path);
        }
    }

    #[test]
    fn invalid_utf8_recovery_keeps_a_future_backup_read_only() {
        let path = settings_path_for_test("utf8-future-backup");
        let future = br#"{"schemaVersion":999,"futureField":"keep exactly"}"#;
        fs::write(&path, b"\xff").unwrap();
        fs::write(backup_path(&path), future).unwrap();
        let loaded = SettingsStore::load_from_path(&path).unwrap();
        assert!(loaded
            .recovery_notice
            .unwrap()
            .message
            .contains(super::schema::READ_ONLY_MESSAGE));
        assert!(SettingsStore::save_to_path(&path, &loaded.settings)
            .unwrap_err()
            .downcast_ref::<super::schema::ReadOnlySettings>()
            .is_some());
        assert_eq!(fs::read(&path).unwrap(), future);
        assert_eq!(fs::read(backup_path(&path)).unwrap(), future);
        assert_eq!(quarantined_bytes(&path), vec![vec![0xff]]);
        cleanup(&path);
    }

    #[cfg(unix)]
    #[test]
    fn permission_denied_is_an_io_error_and_does_not_quarantine_or_overwrite() {
        use std::os::unix::fs::PermissionsExt;
        // Root bypasses these filesystem permissions. CI and the local test
        // container run this test as an unprivileged user.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let path = settings_path_for_test("read-permission");
        let original = b"{\"schemaVersion\":1}";
        fs::write(&path, original).unwrap();
        fs::write(backup_path(&path), b"backup").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
        let error = super::read_settings_text(&path).unwrap_err();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read(backup_path(&path)).unwrap(), b"backup");
        assert!(quarantined_bytes(&path).is_empty());

        // The full loader tightens its own directory permissions. Deny access
        // at an ancestor to exercise a real IO failure before content recovery.
        let parent = path.parent().unwrap();
        let nested = parent.join("nested");
        fs::create_dir(&nested).unwrap();
        let nested_path = nested.join("settings.json");
        fs::write(&nested_path, original).unwrap();
        fs::write(backup_path(&nested_path), b"backup").unwrap();
        fs::set_permissions(parent, fs::Permissions::from_mode(0o0)).unwrap();
        let result = SettingsStore::load_from_path(&nested_path);
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
        let error = result.expect_err("unreadable ancestor must stop the loader");
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(fs::read(&nested_path).unwrap(), original);
        assert_eq!(fs::read(backup_path(&nested_path)).unwrap(), b"backup");
        assert!(quarantined_bytes(&nested_path).is_empty());
        cleanup(&path);
    }

    #[test]
    fn malformed_primary_recovers_from_backup_and_quarantines_the_data() {
        let path = settings_path_for_test("recover-backup");
        let backup_settings = settings_with_channel("backup_channel");
        fs::write(&path, "{\"twitch\":").expect("write malformed primary");
        fs::write(
            backup_path(&path),
            serde_json::to_string(&backup_settings).expect("serialize backup"),
        )
        .expect("write backup");

        let loaded = SettingsStore::load_from_path(&path).expect("recover settings");

        assert_eq!(loaded.settings.twitch.channel_login, "backup_channel");
        assert!(loaded
            .recovery_notice
            .expect("recovery notice")
            .message
            .contains("バックアップから復旧"));
        assert!(path
            .parent()
            .expect("test directory")
            .read_dir()
            .expect("read directory")
            .any(|entry| entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .contains("settings.json.corrupt-")));
        cleanup(&path);
    }

    #[test]
    fn malformed_primary_and_backup_start_with_defaults_and_quarantine_both() {
        let path = settings_path_for_test("recover-default");
        fs::write(&path, "{\"twitch\":").expect("write malformed primary");
        fs::write(backup_path(&path), "{\"speech\":").expect("write malformed backup");

        let loaded = SettingsStore::load_from_path(&path).expect("recover settings");

        assert_eq!(loaded.settings.twitch.channel_login, "");
        assert!(loaded
            .recovery_notice
            .expect("recovery notice")
            .message
            .contains("既定値"));
        let quarantined_count = path
            .parent()
            .expect("test directory")
            .read_dir()
            .expect("read directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
            .count();
        assert_eq!(quarantined_count, 2);
        let primary: AppSettings =
            serde_json::from_str(&fs::read_to_string(&path).expect("read defaults"))
                .expect("defaults must be valid JSON");
        assert_eq!(primary.twitch.channel_login, "");
        cleanup(&path);
    }

    #[test]
    fn failed_patch_validation_keeps_memory_and_disk_unchanged() {
        let path = settings_path_for_test("transaction-validation");
        let previous = settings_with_channel("previous");
        SettingsStore::save_to_path(&path, &previous).expect("save previous settings");
        let original_disk = fs::read_to_string(&path).expect("read previous settings");
        let mut settings = previous;
        let patch: SettingsPatch = serde_json::from_value(serde_json::json!({
            "twitch": { "channelLogin": "candidate" },
            "speech": { "bouyomiSpeed": 301 }
        }))
        .expect("deserialize patch");

        let result = update_settings_transaction(
            &mut settings,
            |candidate| apply_patch(candidate, patch),
            |candidate| {
                SettingsStore::save_to_path(&path, candidate).map_err(|error| error.to_string())
            },
        );

        assert!(result.is_err());
        assert_eq!(settings.twitch.channel_login, "previous");
        assert_eq!(
            fs::read_to_string(&path).expect("read unchanged settings"),
            original_disk
        );
        cleanup(&path);
    }

    #[test]
    fn rule_lists_accept_199_and_200_items_but_reject_201_items() {
        for count in [199, 200] {
            let mut settings = AppSettings::default();
            let patch: SettingsPatch = serde_json::from_value(serde_json::json!({
                "speech": {
                    "blockedWords": (0..count).map(|index| format!("word-{index}")).collect::<Vec<_>>()
                }
            }))
            .expect("deserialize patch");

            apply_patch(&mut settings, patch).expect("rule list within the limit");
            assert_eq!(settings.speech.blocked_words.len(), count);
        }

        let mut settings = AppSettings::default();
        let patch: SettingsPatch = serde_json::from_value(serde_json::json!({
            "speech": {
                "blockedWords": (0..201).map(|index| format!("word-{index}")).collect::<Vec<_>>()
            }
        }))
        .expect("deserialize patch");

        let error = apply_patch(&mut settings, patch).expect_err("201 rules must be rejected");
        assert!(error.contains("1 件超過"));
        assert!(settings.speech.blocked_words.is_empty());
    }

    #[test]
    fn rule_lists_deduplicate_case_variants_using_speech_matching_semantics() {
        let mut settings = AppSettings::default();
        let patch: SettingsPatch = serde_json::from_value(serde_json::json!({
            "speech": {
                "blockedUsers": ["@Alice", "alice", "Bob"],
                "blockedWords": ["BadWord", "badword", "Other"]
            }
        }))
        .expect("deserialize patch");

        apply_patch(&mut settings, patch).expect("apply rule patch");

        assert_eq!(settings.speech.blocked_users, ["Alice", "Bob"]);
        assert_eq!(settings.speech.blocked_words, ["BadWord", "Other"]);
    }

    #[test]
    fn failed_persistence_keeps_memory_and_disk_unchanged() {
        let path = settings_path_for_test("transaction-save");
        let previous = settings_with_channel("previous");
        SettingsStore::save_to_path(&path, &previous).expect("save previous settings");
        let original_disk = fs::read_to_string(&path).expect("read previous settings");
        let mut settings = previous;
        let patch: SettingsPatch = serde_json::from_value(serde_json::json!({
            "twitch": { "channelLogin": "candidate" }
        }))
        .expect("deserialize patch");

        let result = update_settings_transaction(
            &mut settings,
            |candidate| apply_patch(candidate, patch),
            |candidate| {
                SettingsStore::save_to_path_with_fault(&path, candidate, SaveFault::Replace)
                    .map_err(|error| error.to_string())
            },
        );

        assert!(result.is_err());
        assert_eq!(settings.twitch.channel_login, "previous");
        assert_eq!(
            fs::read_to_string(&path).expect("read unchanged settings"),
            original_disk
        );
        cleanup(&path);
    }

    #[test]
    fn launcher_update_commits_memory_only_after_persistence() {
        let path = settings_path_for_test("transaction-launcher");
        let application_path = path.parent().expect("test directory").join("viewer.exe");
        fs::write(&application_path, b"launcher test").expect("create application");
        let mut settings = AppSettings::default();
        SettingsStore::save_to_path(&path, &settings).expect("save initial settings");
        let items = vec![LauncherItem {
            id: "viewer".to_string(),
            kind: LauncherItemKind::Application,
            target: application_path.to_string_lossy().into_owned(),
            display_name: "Viewer".to_string(),
            icon_data_url: None,
            background_color: None,
            group_id: None,
            order: 0,
        }];

        update_settings_transaction(
            &mut settings,
            |candidate| {
                candidate.launcher.items = normalize_launcher_items(items)?;
                Ok(())
            },
            |candidate| {
                SettingsStore::save_to_path(&path, candidate).map_err(|error| error.to_string())
            },
        )
        .expect("commit launcher settings");

        let saved: AppSettings = serde_json::from_str(
            &fs::read_to_string(&path).expect("read persisted launcher settings"),
        )
        .expect("deserialize persisted launcher settings");
        assert_eq!(settings.launcher.items.len(), 1);
        assert_eq!(saved.launcher.items.len(), 1);
        assert_eq!(
            settings.launcher.items[0].target,
            saved.launcher.items[0].target
        );
        cleanup(&path);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn settings_in_app_data_inherit_only_current_user_and_system_acl() {
        use super::write_temp_file;
        let root = PathBuf::from(std::env::var_os("APPDATA").expect("Windows user app data"));
        let directory = root.join(format!(
            "dev.rice.tts-permission-test-{}-{}",
            std::process::id(),
            TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let path = directory.join("settings.json");
        let settings = AppSettings::default();
        SettingsStore::save_to_path(&path, &settings).expect("save in actual user app data");
        SettingsStore::save_to_path(&path, &settings).expect("atomic replace and backup");
        let temporary = write_temp_file(&path, b"test", SaveFault::None).expect("temporary file");
        let check = r#"
$ErrorActionPreference = 'Stop'
$user = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$allowed = @($user, 'S-1-5-18', 'S-1-5-32-544')
foreach ($path in @($env:RICE_ACL_DIRECTORY, $env:RICE_ACL_FILE, $env:RICE_ACL_BACKUP, $env:RICE_ACL_TEMP)) {
  $acl = Get-Acl -LiteralPath $path
  $owner = $acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value
  if ($owner -notin $allowed) { throw "Unexpected owner for $path" }
  foreach ($rule in $acl.Access) {
    $sid = $rule.IdentityReference.Translate([System.Security.Principal.SecurityIdentifier]).Value
    if ($rule.AccessControlType -eq 'Allow' -and $sid -notin $allowed) {
      throw "Unexpected access for $sid on $path"
    }
  }
}
"#;
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", check])
            // A parent PowerShell 7 process exports its module path. Windows
            // PowerShell must rebuild its own compatible built-in module path.
            .env_remove("PSModulePath")
            .env("RICE_ACL_DIRECTORY", &directory)
            .env("RICE_ACL_FILE", &path)
            .env("RICE_ACL_BACKUP", backup_path(&path))
            .env("RICE_ACL_TEMP", &temporary)
            .output()
            .expect("inspect Windows ACLs");
        fs::remove_dir_all(&directory).expect("remove only isolated permission-test directory");
        assert!(
            output.status.success(),
            "Windows user profile ACL: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
