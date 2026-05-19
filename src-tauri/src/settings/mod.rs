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
    pub bouyomi_host: String,
    pub bouyomi_port: u16,
    pub read_user_name: bool,
    pub max_comment_length: u16,
    pub repeat_suppression_seconds: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeechAdapterKind {
    Bouyomi,
}

#[derive(Debug, Default)]
pub struct AppState {
    pub settings: SharedSettings<AppSettings>,
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
                bouyomi_host: "127.0.0.1".to_string(),
                bouyomi_port: 50001,
                read_user_name: true,
                max_comment_length: 120,
                repeat_suppression_seconds: 2,
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
    pub read_user_name: Option<bool>,
    pub max_comment_length: Option<u16>,
    pub repeat_suppression_seconds: Option<u16>,
}

pub struct SettingsStore;

impl SettingsStore {
    #[cfg(feature = "app")]
    pub fn load(app: &tauri::AppHandle) -> anyhow::Result<AppSettings> {
        let path = settings_path(app)?;
        if !path.exists() {
            let settings = AppSettings::default();
            Self::save(app, &settings)?;
            return Ok(settings);
        }

        let text = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    #[cfg(feature = "app")]
    pub fn save(app: &tauri::AppHandle, settings: &AppSettings) -> anyhow::Result<()> {
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
    Ok(state.settings.lock().map_err(|error| error.to_string())?.clone())
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn settings_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<AppSettings, String> {
    let mut settings = state.settings.lock().map_err(|error| error.to_string())?;
    apply_patch(&mut settings, patch)?;
    SettingsStore::save(&app, &settings).map_err(|error| error.to_string())?;
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
        if let Some(read_user_name) = speech.read_user_name {
            settings.speech.read_user_name = read_user_name;
        }
        if let Some(max_length) = speech.max_comment_length {
            settings.speech.max_comment_length = max_length.clamp(1, 500);
        }
        if let Some(seconds) = speech.repeat_suppression_seconds {
            settings.speech.repeat_suppression_seconds = seconds.min(30);
        }
    }

    Ok(())
}

#[cfg(feature = "app")]
fn settings_path(app: &tauri::AppHandle) -> anyhow::Result<std::path::PathBuf> {
    Ok(app.path().app_data_dir()?.join("settings.json"))
}
