use crate::launcher::{LauncherSettings, LauncherSettingsPatch};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub twitch: TwitchSettings,
    pub speech: SpeechSettings,
    #[serde(default)]
    pub launcher: LauncherSettings,
    #[serde(default)]
    pub window: WindowSettings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct WindowSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<WindowPosition>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
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
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
#[serde(rename_all = "camelCase")]
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
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SpeechAdapterKind {
    Bouyomi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
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
    std::env::var("RICE_BOUYOMI_HOST")
        .ok()
        .and_then(|host| crate::speech::endpoint::validate_bouyomi_host(&host).ok())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

pub(crate) fn default_twitch_client_id() -> String {
    option_env!("RICE_TWITCH_CLIENT_ID")
        .unwrap_or("")
        .trim()
        .to_string()
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
