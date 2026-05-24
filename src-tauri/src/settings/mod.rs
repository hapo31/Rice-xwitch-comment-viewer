#[cfg(feature = "app")]
use crate::app_events::{emit_app_log, AppLogLevel};
use crate::speech::SpeechQueueState;
use crate::twitch::TwitchAuthState;
#[cfg(feature = "app")]
use crate::twitch::TwitchConnectionHandle;
use crate::SharedSettings;
use serde::{Deserialize, Serialize};
#[cfg(feature = "app")]
use std::fs;
#[cfg(feature = "app")]
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub twitch: TwitchSettings,
    pub speech: SpeechSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchSettings {
    pub channel_login: String,
    pub auto_connect: bool,
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
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub twitch: Option<TwitchSettingsPatch>,
    pub speech: Option<SpeechSettingsPatch>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchSettingsPatch {
    pub channel_login: Option<String>,
    pub auto_connect: Option<bool>,
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

impl SettingsStore {
    #[cfg(feature = "app")]
    pub fn load<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<AppSettings> {
        let path = settings_path(app)?;
        if !path.exists() {
            let settings = AppSettings::default();
            Self::save(app, &settings)?;
            return Ok(settings);
        }

        let text = fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(Into::into)
    }

    #[cfg(feature = "app")]
    pub fn save<R: tauri::Runtime>(
        app: &tauri::AppHandle<R>,
        settings: &AppSettings,
    ) -> anyhow::Result<()> {
        let path = settings_path(app)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let text = serde_json::to_string_pretty(settings)?;
        fs::write(path, text)?;
        Ok(())
    }
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
pub fn settings_update(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<AppSettings, String> {
    let mut settings = state.settings.lock().map_err(|error| error.to_string())?;
    apply_patch(&mut settings, patch)?;
    SettingsStore::save(&app, &settings).map_err(|error| error.to_string())?;
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
    }

    if let Some(speech) = patch.speech {
        if let Some(adapter) = speech.adapter {
            settings.speech.adapter = adapter;
        }
        if let Some(host) = speech.bouyomi_host {
            settings.speech.bouyomi_host = host.trim().to_string();
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
            settings.speech.blocked_users = normalize_rule_list(blocked_users);
        }
        if let Some(blocked_words) = speech.blocked_words {
            settings.speech.blocked_words = normalize_rule_list(blocked_words);
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

    Ok(())
}

fn normalize_rule_list(items: Vec<String>) -> Vec<String> {
    let mut normalized = items
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized.truncate(200);
    normalized
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
