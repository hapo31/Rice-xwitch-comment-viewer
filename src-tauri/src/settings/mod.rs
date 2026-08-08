#[cfg(feature = "app")]
use crate::app_events::{emit_app_log, AppLogLevel};
use crate::launcher::{normalize_launcher_items, LauncherSettings, LauncherSettingsPatch};
use crate::speech::SpeechQueueState;
use crate::twitch::TwitchAuthState;
#[cfg(feature = "app")]
use crate::twitch::TwitchConnectionHandle;
use crate::SharedSettings;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "app")]
use tauri::Manager;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub twitch: TwitchSettings,
    pub speech: SpeechSettings,
    #[serde(default)]
    pub launcher: LauncherSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
pub struct SpeechSettings {
    pub adapter: SpeechAdapterKind,
    #[serde(default = "default_bouyomi_host")]
    pub bouyomi_host: String,
    pub bouyomi_port: u16,
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
pub enum SpeechAdapterKind {
    Bouyomi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    std::env::var("RICE_BOUYOMI_HOST").unwrap_or_else(|_| "127.0.0.1".to_string())
}

pub(crate) fn default_twitch_client_id() -> String {
    option_env!("RICE_TWITCH_CLIENT_ID")
        .unwrap_or("")
        .trim()
        .to_string()
}

#[derive(Debug, Default)]
pub struct AppState {
    pub settings: SharedSettings<AppSettings>,
    pub settings_recovery_notice: SharedSettings<Option<SettingsRecoveryNotice>>,
    pub twitch_auth: SharedSettings<TwitchAuthState>,
    pub speech_queue: SharedSettings<SpeechQueueState>,
    #[cfg(feature = "app")]
    pub twitch_connection: SharedSettings<Option<TwitchConnectionHandle>>,
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
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub twitch: Option<TwitchSettingsPatch>,
    pub speech: Option<SpeechSettingsPatch>,
    pub launcher: Option<LauncherSettingsPatch>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchSettingsPatch {
    pub channel_login: Option<String>,
    pub auto_connect: Option<bool>,
    pub confirm_before_stop_chat: Option<bool>,
    pub live_chat_announcements: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechSettingsPatch {
    pub adapter: Option<SpeechAdapterKind>,
    pub bouyomi_host: Option<String>,
    pub bouyomi_port: Option<u16>,
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
pub struct SettingsRecoveryNotice {
    pub message: String,
}

#[derive(Debug)]
pub struct LoadedSettings {
    pub settings: AppSettings,
    pub recovery_notice: Option<SettingsRecoveryNotice>,
}

impl SettingsStore {
    #[cfg(feature = "app")]
    pub fn load<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<LoadedSettings> {
        let path = settings_path(app)?;
        Self::load_from_path(&path)
    }

    fn load_from_path(path: &Path) -> anyhow::Result<LoadedSettings> {
        if !path.exists() {
            let settings = AppSettings::default();
            Self::save_to_path(path, &settings)?;
            return Ok(LoadedSettings {
                settings,
                recovery_notice: None,
            });
        }

        let text = fs::read_to_string(path)?;
        match serde_json::from_str(&text) {
            Ok(settings) => Ok(LoadedSettings {
                settings,
                recovery_notice: None,
            }),
            Err(_) => Self::recover_from_invalid_primary(path),
        }
    }

    #[cfg(feature = "app")]
    pub fn save<R: tauri::Runtime>(
        app: &tauri::AppHandle<R>,
        settings: &AppSettings,
    ) -> anyhow::Result<()> {
        let path = settings_path(app)?;
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
        let text = serde_json::to_string_pretty(settings)?;
        Self::save_text_to_path(path, &text, fault)
    }

    fn save_text_to_path(path: &Path, text: &str, fault: SaveFault) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temporary_path = write_temp_file(path, text.as_bytes(), fault)?;
        let result = (|| {
            if path.exists() {
                let previous = fs::read_to_string(path)?;
                serde_json::from_str::<AppSettings>(&previous).map_err(|error| {
                    anyhow::anyhow!("既存の設定をバックアップできませんでした: {error}")
                })?;
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

    fn recover_from_invalid_primary(path: &Path) -> anyhow::Result<LoadedSettings> {
        let corrupted_primary = quarantine_file(path)?;
        let backup = backup_path(path);

        if backup.exists() {
            let backup_text = fs::read_to_string(&backup)?;
            if let Ok(settings) = serde_json::from_str::<AppSettings>(&backup_text) {
                atomic_write(path, backup_text.as_bytes(), SaveFault::None)?;
                return Ok(LoadedSettings {
                    settings,
                    recovery_notice: Some(SettingsRecoveryNotice {
                        message: format!(
                            "設定ファイルが破損していたため、バックアップから復旧しました。破損ファイル: {}",
                            corrupted_primary.display()
                        ),
                    }),
                });
            }

            let corrupted_backup = quarantine_file(&backup)?;
            let settings = AppSettings::default();
            Self::save_to_path(path, &settings)?;
            return Ok(LoadedSettings {
                settings,
                recovery_notice: Some(SettingsRecoveryNotice {
                    message: format!(
                        "設定ファイルとバックアップが破損していたため、既定値で起動しました。退避先: {}, {}",
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
                    "設定ファイルが破損していたため、既定値で起動しました。破損ファイル: {}",
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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
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
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
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
    if fault == SaveFault::Replace {
        return Err(anyhow::anyhow!("fault injected: atomic replace failed"));
    }

    atomic_replace(source, destination)?;
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

pub(crate) fn update_settings_transaction(
    settings: &mut AppSettings,
    update: impl FnOnce(&mut AppSettings) -> Result<(), String>,
    save: impl FnOnce(&AppSettings) -> Result<(), String>,
) -> Result<(), String> {
    let mut candidate = settings.clone();
    update(&mut candidate)?;
    save(&candidate)?;
    *settings = candidate;
    Ok(())
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn settings_update(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<AppSettings, String> {
    let mut settings = state.settings.lock().map_err(|error| error.to_string())?;
    update_settings_transaction(
        &mut settings,
        |candidate| apply_patch(candidate, patch),
        |candidate| SettingsStore::save(&app, candidate).map_err(|error| error.to_string()),
    )?;
    emit_app_log(&app, AppLogLevel::Info, "設定を保存しました。");
    Ok(settings.clone())
}

fn apply_patch(settings: &mut AppSettings, patch: SettingsPatch) -> Result<(), String> {
    if let Some(twitch) = patch.twitch {
        if let Some(channel_login) = twitch.channel_login {
            settings.twitch.channel_login = channel_login.trim().to_string();
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
            settings.speech.repeat_suppression_seconds = seconds.min(30);
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
            settings.launcher.items = normalize_launcher_items(items)?;
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
        SettingsPatch, SettingsStore,
    };
    use crate::launcher::{normalize_launcher_items, LauncherItem, LauncherItemKind};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn settings_path_for_test(name: &str) -> PathBuf {
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

    fn cleanup(path: &std::path::Path) {
        fs::remove_dir_all(path.parent().expect("test path parent"))
            .expect("remove test directory");
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
    fn malformed_primary_recovers_from_backup_and_quarantines_the_data() {
        let path = settings_path_for_test("recover-backup");
        let backup_settings = settings_with_channel("backup-channel");
        fs::write(&path, "{\"twitch\":").expect("write malformed primary");
        fs::write(
            backup_path(&path),
            serde_json::to_string(&backup_settings).expect("serialize backup"),
        )
        .expect("write backup");

        let loaded = SettingsStore::load_from_path(&path).expect("recover settings");

        assert_eq!(loaded.settings.twitch.channel_login, "backup-channel");
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
}
