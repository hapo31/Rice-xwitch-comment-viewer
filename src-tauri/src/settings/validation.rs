//! Authoritative, bounded domain values shared by commands and schema migration.
use super::{AppSettings, SettingsPatch, SpeechSettings};
use serde::Serialize;

pub const MAX_RULES: usize = 200;
pub const MAX_RULE_BYTES: usize = 64 * 1024;
pub const MAX_WORD_CHARS: usize = 500;
pub const MAX_WORD_BYTES: usize = 2048;
pub const MAX_PATCH_BYTES: usize = 256 * 1024;

/// Inspect framework-owned strings before cloning the wire DTO.
pub fn preflight_wire(patch: &serde_json::Value) -> Result<(), ValidationError> {
    fn keys(
        value: &serde_json::Value,
        allowed: &[&str],
        field: &'static str,
    ) -> Result<(), ValidationError> {
        if value
            .as_object()
            .is_some_and(|object| object.keys().any(|key| !allowed.contains(&key.as_str())))
        {
            return Err(ValidationError::new(
                field,
                "unknownField",
                "未知の設定項目があります。項目名を確認してください。",
            ));
        }
        Ok(())
    }
    keys(patch, &["twitch", "speech", "launcher"], "patch")?;
    let twitch = &patch["twitch"];
    keys(
        twitch,
        &[
            "channelLogin",
            "autoConnect",
            "confirmBeforeStopChat",
            "liveChatAnnouncements",
        ],
        "twitch",
    )?;
    if let Some(login) = twitch["channelLogin"].as_str() {
        TwitchLogin::parse(login, true)?;
    }
    let speech = &patch["speech"];
    keys(
        speech,
        &[
            "adapter",
            "bouyomiHost",
            "bouyomiPort",
            "bouyomiRemoteMode",
            "bouyomiSpeed",
            "bouyomiTone",
            "bouyomiVolume",
            "bouyomiVoice",
            "readUserName",
            "autoSpeak",
            "maxCommentLength",
            "repeatSuppressionSeconds",
            "blockedUsers",
            "blockedWords",
            "urlHandling",
            "readEmotes",
            "connectionSuccessSpeechEnabled",
            "connectionSuccessSpeechText",
        ],
        "speech",
    )?;
    if let Some(host) = speech["bouyomiHost"].as_str() {
        crate::speech::endpoint::validate_bouyomi_host(host).map_err(|message| {
            ValidationError::new("speech.bouyomiHost", "invalidEndpoint", message)
        })?;
    }
    if let Some(text) = speech["connectionSuccessSpeechText"].as_str() {
        if text.len() > 480 || text.chars().count() > 120 || text.chars().any(char::is_control) {
            return Err(ValidationError::new(
                "speech.connectionSuccessSpeechText",
                "tooLong",
                "接続時の読み上げ文は120文字・480 UTF-8バイト以内にしてください。",
            ));
        }
    }
    let mut total = 0;
    for (name, field) in [
        ("blockedUsers", "speech.blockedUsers"),
        ("blockedWords", "speech.blockedWords"),
    ] {
        if let Some(values) = speech[name].as_array() {
            if values.len() > MAX_RULES {
                return Err(ValidationError::new(
                    field,
                    "tooManyRules",
                    "NGルールは各200件以内にしてください。",
                ));
            }
            for value in values {
                let Some(value) = value.as_str() else {
                    continue;
                }; // serde rejects wrong wire types
                if name == "blockedWords" {
                    validate_word(value)?;
                } else {
                    if value.len() > 128 || value.chars().any(char::is_control) {
                        return Err(ValidationError::new(
                            field,
                            "invalidRule",
                            "NGユーザーの長さまたは文字が無効です。",
                        ));
                    }
                    TwitchLogin::parse(value.trim().trim_start_matches('@'), false).map_err(
                        |_| {
                            ValidationError::new(
                                field,
                                "invalidRule",
                                "NGユーザーは英数字・_の3〜25文字で入力してください。",
                            )
                        },
                    )?;
                }
                total += value.len();
                if total > MAX_RULE_BYTES {
                    return Err(ValidationError::new(
                        field,
                        "rulesByteLimit",
                        "NGルール合計は64KiB UTF-8以内にしてください。",
                    ));
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct ValidationError {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
    pub recovery: &'static str,
}
impl ValidationError {
    pub fn new(field: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            code,
            message: message.into(),
            recovery: "該当する入力欄を修正してください。保存済みの設定は変更していません。",
        }
    }
}
impl From<String> for ValidationError {
    fn from(message: String) -> Self {
        Self::new("request", "operationFailed", message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwitchLogin(String);
impl TwitchLogin {
    pub fn parse(value: &str, allow_empty: bool) -> Result<Self, ValidationError> {
        let login = value.trim();
        if value.len() > 128
            || value.chars().any(char::is_control)
            || (!login.is_empty()
                && (!(3..=25).contains(&login.len())
                    || !login
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')))
            || (login.is_empty() && !allow_empty)
        {
            return Err(ValidationError::new(
                "twitch.channelLogin",
                "invalidLogin",
                "Twitchチャンネルは英数字・_の3〜25文字で入力してください。設定の空欄は自分のチャンネルを使います。",
            ));
        }
        Ok(Self(login.to_ascii_lowercase()))
    }
    pub fn into_string(self) -> String {
        self.0
    }
}

pub fn validate_word(value: &str) -> Result<(), ValidationError> {
    if value.len() > MAX_WORD_BYTES
        || value.chars().count() > MAX_WORD_CHARS
        || value.chars().any(char::is_control)
        || value.trim().is_empty()
    {
        return Err(ValidationError::new(
            "speech.blockedWords",
            "invalidRule",
            "NGワードは制御文字を含まない1〜500文字・2048 UTF-8バイト以内にしてください。",
        ));
    }
    Ok(())
}
pub fn validate_rules(users: &[String], words: &[String]) -> Result<(), ValidationError> {
    for (field, values) in [
        ("speech.blockedUsers", users),
        ("speech.blockedWords", words),
    ] {
        if values.len() > MAX_RULES {
            return Err(ValidationError::new(
                field,
                "tooManyRules",
                "NGルールは各200件以内にしてください。",
            ));
        }
    }
    for user in users {
        TwitchLogin::parse(user, false).map_err(|_| {
            ValidationError::new(
                "speech.blockedUsers",
                "invalidRule",
                "NGユーザーはTwitch login（英数字・_の3〜25文字）で入力してください。",
            )
        })?;
    }
    for word in words {
        validate_word(word)?;
    }
    let bytes = users.iter().chain(words).map(String::len).sum::<usize>();
    if bytes > MAX_RULE_BYTES {
        return Err(ValidationError::new(
            "speech.blockedWords",
            "rulesByteLimit",
            "NGユーザーとNGワードの合計は64KiB UTF-8以内にしてください。",
        ));
    }
    Ok(())
}
pub fn validate_speech(settings: &SpeechSettings) -> Result<(), ValidationError> {
    if settings.bouyomi_port == 0 {
        return Err(ValidationError::new(
            "speech.bouyomiPort",
            "outOfRange",
            "棒読みちゃんのポートは1〜65535で入力してください。",
        ));
    }
    crate::speech::endpoint::BouyomiAddress::new(&settings.bouyomi_host, settings.bouyomi_port)
        .map_err(|message| {
            ValidationError::new("speech.bouyomiHost", "invalidEndpoint", message)
        })?;
    for (field, value, min, max) in [
        ("speech.bouyomiSpeed", settings.bouyomi_speed, -1, 300),
        ("speech.bouyomiTone", settings.bouyomi_tone, -1, 200),
        ("speech.bouyomiVolume", settings.bouyomi_volume, -1, 100),
        ("speech.bouyomiVoice", settings.bouyomi_voice, 0, 30000),
    ] {
        if !(min..=max).contains(&value) {
            return Err(ValidationError::new(
                field,
                "outOfRange",
                format!("{field}は{min}〜{max}の範囲で入力してください。"),
            ));
        }
    }
    if !(1..=500).contains(&settings.max_comment_length) {
        return Err(ValidationError::new(
            "speech.maxCommentLength",
            "outOfRange",
            "最大コメント長は1〜500文字で入力してください。",
        ));
    }
    if settings.repeat_suppression_seconds > 30 {
        return Err(ValidationError::new(
            "speech.repeatSuppressionSeconds",
            "outOfRange",
            "連投抑制秒は0〜30の範囲で入力してください。",
        ));
    }
    let text = &settings.connection_success_speech_text;
    if text.len() > 480 || text.chars().count() > 120 || text.chars().any(char::is_control) {
        return Err(ValidationError::new(
            "speech.connectionSuccessSpeechText",
            "tooLong",
            "接続時の読み上げ文は制御文字を含まない120文字・480 UTF-8バイト以内にしてください。",
        ));
    }
    validate_rules(&settings.blocked_users, &settings.blocked_words)
}
pub fn validate_settings(settings: &AppSettings) -> Result<(), ValidationError> {
    TwitchLogin::parse(&settings.twitch.channel_login, true)?;
    validate_speech(&settings.speech)?;
    crate::launcher::validate_launcher_structure(&settings.launcher.items)
        .map_err(|message| ValidationError::new("launcher.items", "invalidLauncher", message))
}

/// Check unnormalized wire values before any trim/dedup/clamp can hide invalid input.
pub fn validate_patch(patch: &SettingsPatch) -> Result<(), ValidationError> {
    if let Some(twitch) = &patch.twitch {
        if let Some(login) = &twitch.channel_login {
            TwitchLogin::parse(login, true)?;
        }
    }
    if let Some(speech) = &patch.speech {
        if let Some(host) = &speech.bouyomi_host {
            crate::speech::endpoint::validate_bouyomi_host(host).map_err(|message| {
                ValidationError::new("speech.bouyomiHost", "invalidEndpoint", message)
            })?;
        }
        if let Some(text) = &speech.connection_success_speech_text {
            if text.len() > 480 || text.chars().count() > 120 || text.chars().any(char::is_control)
            {
                return Err(ValidationError::new(
                    "speech.connectionSuccessSpeechText",
                    "tooLong",
                    "接続時の読み上げ文は120文字・480 UTF-8バイト以内にしてください。",
                ));
            }
        }
        for (field, values) in [
            ("speech.blockedUsers", &speech.blocked_users),
            ("speech.blockedWords", &speech.blocked_words),
        ] {
            if let Some(values) = values {
                if values.len() > MAX_RULES
                    || values.iter().map(String::len).sum::<usize>() > MAX_RULE_BYTES
                {
                    return Err(ValidationError::new(
                        field,
                        "rulePayloadLimit",
                        "NGルールは各200件・合計64KiB UTF-8以内にしてください。",
                    ));
                }
                for value in values {
                    if field == "speech.blockedWords" {
                        validate_word(value)?;
                    } else {
                        if value.len() > 128 || value.chars().any(char::is_control) {
                            return Err(ValidationError::new(
                                field,
                                "invalidRule",
                                "NGユーザーの長さまたは文字が無効です。",
                            ));
                        }
                        TwitchLogin::parse(value.trim().trim_start_matches('@'), false).map_err(
                            |_| {
                                ValidationError::new(
                                    field,
                                    "invalidRule",
                                    "NGユーザーは英数字・_の3〜25文字で入力してください。",
                                )
                            },
                        )?;
                    }
                }
            }
        }
        let mut candidate = AppSettings::default().speech;
        if let Some(host) = &speech.bouyomi_host {
            candidate.bouyomi_host = host.clone();
        }
        if let Some(value) = speech.bouyomi_port {
            candidate.bouyomi_port = value;
        }
        if let Some(value) = speech.bouyomi_speed {
            candidate.bouyomi_speed = value;
        }
        if let Some(value) = speech.bouyomi_tone {
            candidate.bouyomi_tone = value;
        }
        if let Some(value) = speech.bouyomi_volume {
            candidate.bouyomi_volume = value;
        }
        if let Some(value) = speech.bouyomi_voice {
            candidate.bouyomi_voice = value;
        }
        if let Some(value) = speech.max_comment_length {
            candidate.max_comment_length = value;
        }
        if let Some(value) = speech.repeat_suppression_seconds {
            candidate.repeat_suppression_seconds = value;
        }
        if let Some(value) = &speech.connection_success_speech_text {
            candidate.connection_success_speech_text = value.clone();
        }
        if let Some(values) = &speech.blocked_users {
            if values.len() > MAX_RULES {
                return Err(ValidationError::new(
                    "speech.blockedUsers",
                    "tooManyRules",
                    "NGユーザーは200件以内にしてください。",
                ));
            }
            candidate.blocked_users = values
                .iter()
                .map(|value| value.trim().trim_start_matches('@').to_owned())
                .collect();
        }
        if let Some(values) = &speech.blocked_words {
            candidate.blocked_words = values.clone();
        }
        validate_speech(&candidate)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backend_and_forms_share_exact_utf8_and_collection_boundaries() {
        let fixtures: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/tauri/fixtures/settings-validation.json"
        ))
        .unwrap();
        for fixture in fixtures.as_array().unwrap() {
            let value = fixture["value"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    format!(
                        "{}{}",
                        fixture["unit"]
                            .as_str()
                            .unwrap_or("")
                            .repeat(fixture["repeat"].as_u64().unwrap_or(1) as usize),
                        fixture["suffix"].as_str().unwrap_or("")
                    )
                });
            let valid = match fixture["kind"].as_str().unwrap() {
                "login" => TwitchLogin::parse(&value, true).is_ok(),
                "host" => crate::speech::endpoint::validate_bouyomi_host(&value).is_ok(),
                "word" => validate_word(&value).is_ok(),
                "confirmation" => {
                    let mut speech = AppSettings::default().speech;
                    speech.connection_success_speech_text = value;
                    validate_speech(&speech).is_ok()
                }
                "rules" => validate_rules(
                    &[],
                    &vec![value; fixture["count"].as_u64().unwrap() as usize],
                )
                .is_ok(),
                _ => panic!("unknown shared fixture"),
            };
            assert_eq!(valid, fixture["valid"].as_bool().unwrap(), "{fixture}");
        }
    }
    #[test]
    fn structured_fields_and_combined_rule_quota_are_bounded() {
        let mut settings = AppSettings::default();
        for (field, value) in [
            ("channelLogin", "ab"),
            ("channelLogin", "@valid"),
            ("channelLogin", "valid\n"),
        ] {
            let patch: SettingsPatch =
                serde_json::from_value(serde_json::json!({"twitch": {field: value}})).unwrap();
            let error = validate_patch(&patch).unwrap_err();
            assert_eq!(error.field, "twitch.channelLogin");
            assert_eq!(
                serde_json::to_value(&error).unwrap()["code"],
                "invalidLogin"
            );
        }
        settings.speech.blocked_words = vec!["x".repeat(500); 131];
        validate_settings(&settings).unwrap();
        settings.speech.blocked_words.push("x".repeat(500));
        assert_eq!(
            validate_settings(&settings).unwrap_err().code,
            "rulesByteLimit"
        );
    }
}
