//! Persistence wire is optional and versioned; command/domain DTOs stay strict.
//! Borrow raw fields and cap arrays before allocating values, not an 8MiB Value tree.
use super::{AppSettings, SettingsPatch, WindowPosition};
use crate::launcher::{
    LauncherItem, LauncherItemKind, normalize_launcher_icon_data_url, validate_launcher_structure,
};
use serde::de::{DeserializeOwned, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserializer, Serialize};
use serde_json::{Value, value::RawValue};
use std::fmt;

pub(super) const CURRENT_VERSION: u64 = 1;
pub(super) const READ_ONLY_MESSAGE: &str = "この設定には未対応の版・項目があるため保存しません。元ファイルを残したまま対応するRiceの版で開いてください。既定値でやり直す場合は終了後に設定本体とバックアップを別の場所へコピーしてから移動してください。";

#[derive(Debug, thiserror::Error)]
#[error("{READ_ONLY_MESSAGE}")]
pub(super) struct ReadOnlySettings;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PersistedSettings<'a> {
    schema_version: u64,
    #[serde(flatten)]
    settings: &'a AppSettings,
}
impl<'a> PersistedSettings<'a> {
    pub(super) fn new(settings: &'a AppSettings) -> Self {
        Self {
            schema_version: CURRENT_VERSION,
            settings,
        }
    }
}

pub(super) struct DecodedSettings {
    pub settings: AppSettings,
    pub needs_resave: bool,
    pub read_only: bool,
    pub unsupported_version: bool,
}

#[derive(Default)]
struct Fields<'a> {
    values: Vec<(&'static str, &'a RawValue)>,
    unknown: bool,
    ambiguous_version: bool,
}
impl<'a> Fields<'a> {
    fn get(&self, name: &str) -> Option<&'a RawValue> {
        self.values
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| *value)
    }
}
struct ObjectVisitor(&'static [&'static str]);
impl<'de> Visitor<'de> for ObjectVisitor {
    type Value = Fields<'de>;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an optional settings object")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let mut fields = Fields {
            values: Vec::with_capacity(self.0.len()),
            unknown: false,
            ambiguous_version: false,
        };
        while let Some(key) = map.next_key::<String>()? {
            if let Some(&known) = self.0.iter().find(|&&known| known == key) {
                let value = map.next_value::<&'de RawValue>()?;
                if fields.get(known).is_some() {
                    // Duplicate known fields are ambiguous; never silently rewrite them.
                    fields.unknown = true;
                    fields.ambiguous_version |= known == "schemaVersion";
                } else {
                    fields.values.push((known, value));
                }
            } else {
                fields.unknown = true;
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(fields)
    }
}
fn fields<'a>(
    raw: Option<&'a RawValue>,
    names: &'static [&'static str],
    changed: &mut bool,
) -> Fields<'a> {
    let Some(raw) = raw.filter(|raw| raw.get() != "null") else {
        return Fields::default();
    };
    match serde_json::Deserializer::from_str(raw.get()).deserialize_map(ObjectVisitor(names)) {
        Ok(fields) => fields,
        Err(_) => {
            *changed = true;
            // Raw JSON can preserve key escapes that an owned String cannot
            // decode. Don't discard an uninterpretable object's fields.
            Fields {
                unknown: raw.get().starts_with('{'),
                ambiguous_version: raw.get().starts_with('{') && names.contains(&"schemaVersion"),
                ..Fields::default()
            }
        }
    }
}

struct ArrayVisitor(usize);
impl<'de> Visitor<'de> for ArrayVisitor {
    type Value = Vec<&'de RawValue>;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded settings array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(self.0));
        while let Some(value) = sequence.next_element::<&'de RawValue>()? {
            if values.len() == self.0 {
                return Err(serde::de::Error::custom("settings array limit"));
            }
            values.push(value);
        }
        Ok(values)
    }
}
fn array(raw: &RawValue, maximum: usize) -> Option<Vec<&RawValue>> {
    serde_json::Deserializer::from_str(raw.get())
        .deserialize_seq(ArrayVisitor(maximum))
        .ok()
}

fn scalar<T: DeserializeOwned>(
    raw: Option<&RawValue>,
    maximum: usize,
    changed: &mut bool,
) -> Option<T> {
    let raw = raw.filter(|raw| raw.get() != "null")?;
    // A JSON-escaped valid string needs at most six bytes per UTF-8 byte.
    if raw.get().len() <= maximum {
        if let Ok(value) = serde_json::from_str(raw.get()) {
            return Some(value);
        }
    }
    *changed = true;
    None
}
fn string(raw: Option<&RawValue>, maximum: usize, changed: &mut bool) -> Option<String> {
    scalar(raw, maximum * 6 + 2, changed)
}

const TWITCH: &[&str] = &[
    "channelLogin",
    "autoConnect",
    "confirmBeforeStopChat",
    "liveChatAnnouncements",
];
const SPEECH: &[&str] = &[
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
];

fn patch_value(
    settings: &mut AppSettings,
    section: &str,
    key: &str,
    value: Value,
    changed: &mut bool,
) {
    let patch: Result<SettingsPatch, _> =
        serde_json::from_value(serde_json::json!({section: {key: value.clone()}}));
    if let Ok(patch) = patch {
        if super::apply_validated_settings_patch(settings, patch, |_| Ok(())).is_ok() {
            // Preserve command normalization (trim/login case/rule dedup), not a second dialect.
            if serde_json::to_value(&*settings).expect("settings serialize")[section][key] != value
            {
                *changed = true;
            }
            return;
        }
    }
    *changed = true;
}
fn scalar_patch(
    settings: &mut AppSettings,
    section: &str,
    key: &str,
    raw: Option<&RawValue>,
    changed: &mut bool,
) {
    let maximum = match key {
        "channelLogin" => 128 * 6 + 2,
        "bouyomiHost" => 253 * 6 + 2,
        "connectionSuccessSpeechText" => 480 * 6 + 2,
        _ => 64,
    };
    let Some(raw) = raw.filter(|raw| raw.get() != "null") else {
        return;
    };
    // Scalars cannot amplify into a nested/large Value tree.
    if raw.get().starts_with(['{', '[']) {
        *changed = true;
        return;
    }
    if let Some(value) = scalar::<Value>(Some(raw), maximum, changed) {
        patch_value(settings, section, key, value, changed);
    }
}
fn rules_patch(settings: &mut AppSettings, key: &str, raw: Option<&RawValue>, changed: &mut bool) {
    let Some(raw) = raw.filter(|raw| raw.get() != "null") else {
        return;
    };
    let Some(raw_values) = array(raw, super::validation::MAX_RULES) else {
        *changed = true;
        return;
    };
    let maximum = if key == "blockedUsers" {
        128
    } else {
        super::validation::MAX_WORD_BYTES
    };
    let mut values = Vec::with_capacity(raw_values.len());
    for raw in raw_values {
        let Some(value) = string(Some(raw), maximum, changed) else {
            *changed = true;
            return;
        };
        values.push(value);
    }
    patch_value(
        settings,
        "speech",
        key,
        serde_json::to_value(values).expect("rules serialize"),
        changed,
    );
}

fn launcher_item(raw: &RawValue, changed: &mut bool, unknown: &mut bool) -> Option<LauncherItem> {
    let item = fields(
        Some(raw),
        &[
            "id",
            "kind",
            "target",
            "displayName",
            "iconDataUrl",
            "backgroundColor",
            "groupId",
            "order",
        ],
        changed,
    );
    *unknown |= item.unknown;
    let id = string(item.get("id"), 64, changed)?;
    let target = string(item.get("target"), 4096, changed)?;
    let mut candidate = LauncherItem {
        id,
        target,
        kind: scalar(item.get("kind"), 64, changed).unwrap_or(LauncherItemKind::Application),
        display_name: "アプリ".into(),
        icon_data_url: None,
        background_color: None,
        group_id: None,
        order: scalar(item.get("order"), 64, changed).unwrap_or(0),
    };
    // A missing/invalid identity or target has no safe invented default: omit that item.
    if validate_launcher_structure(std::slice::from_ref(&candidate)).is_err() {
        *changed = true;
        return None;
    }
    for key in ["displayName", "backgroundColor", "groupId"] {
        let maximum = if key == "displayName" { 480 } else { 256 };
        if let Some(value) = string(item.get(key), maximum, changed) {
            if value.trim().is_empty() {
                *changed = true;
                continue;
            }
            let mut next = candidate.clone();
            match key {
                "displayName" => next.display_name = value,
                "backgroundColor" => next.background_color = Some(value),
                _ => next.group_id = Some(value),
            }
            if validate_launcher_structure(std::slice::from_ref(&next)).is_ok() {
                candidate = next;
            } else {
                *changed = true;
            }
        }
    }
    let original = string(item.get("iconDataUrl"), 64 * 1024 + 22, changed);
    candidate.icon_data_url = normalize_launcher_icon_data_url(original.clone());
    *changed |= candidate.icon_data_url != original;
    // Match normal metadata normalization, without filesystem/COM access.
    for text in [&mut candidate.target, &mut candidate.display_name] {
        if text.trim().len() != text.len() {
            *text = text.trim().to_string();
            *changed = true;
        }
    }
    for text in [&mut candidate.background_color, &mut candidate.group_id]
        .into_iter()
        .flatten()
    {
        if text.trim().len() != text.len() {
            *text = text.trim().to_string();
            *changed = true;
        }
    }
    Some(candidate)
}

pub(super) fn decode(text: &str) -> Result<DecodedSettings, String> {
    crate::resource_limits::check_bytes(
        text.len(),
        crate::resource_limits::MAX_SETTINGS_JSON_BYTES,
        "設定JSON",
    )
    .map_err(|error| error.to_string())?;
    let raw: &RawValue =
        serde_json::from_str(text).map_err(|_| "設定JSONを読み取れません。".to_string())?;
    let mut changed = false;
    let root = fields(
        Some(raw),
        &["schemaVersion", "twitch", "speech", "launcher", "window"],
        &mut changed,
    );
    let version = match root.get("schemaVersion") {
        None => Some(0),
        Some(raw) if raw.get() == "null" => Some(0),
        Some(raw) => serde_json::from_str::<u64>(raw.get()).ok(),
    };
    if root.ambiguous_version || version.is_none_or(|version| version > CURRENT_VERSION) {
        // Future semantics cannot be guessed; no automatic connection/settings mutation.
        return Ok(DecodedSettings {
            settings: AppSettings::default(),
            needs_resave: false,
            read_only: true,
            unsupported_version: true,
        });
    }
    let mut version = version.expect("supported version");
    while version < CURRENT_VERSION {
        match version {
            // The only released predecessor is the unversioned v0 document.
            // v0 -> v1 makes every field optional and completes validated defaults.
            0 => version = 1,
            _ => unreachable!("unsupported migration step"),
        }
        changed = true;
    }
    let mut settings = AppSettings::default();
    let twitch = fields(root.get("twitch"), TWITCH, &mut changed);
    let speech = fields(root.get("speech"), SPEECH, &mut changed);
    let mut unknown = root.unknown || twitch.unknown || speech.unknown;
    for &key in TWITCH {
        scalar_patch(&mut settings, "twitch", key, twitch.get(key), &mut changed);
    }
    for &key in SPEECH {
        if !matches!(key, "blockedUsers" | "blockedWords") {
            scalar_patch(&mut settings, "speech", key, speech.get(key), &mut changed);
        }
    }
    for key in ["blockedUsers", "blockedWords"] {
        rules_patch(&mut settings, key, speech.get(key), &mut changed);
    }
    let launcher = fields(root.get("launcher"), &["items"], &mut changed);
    unknown |= launcher.unknown;
    if let Some(raw) = launcher.get("items").filter(|raw| raw.get() != "null") {
        if let Some(values) = array(raw, 200) {
            for raw in values {
                if let Some(item) = launcher_item(raw, &mut changed, &mut unknown) {
                    settings.launcher.items.push(item);
                } else {
                    changed = true;
                }
            }
            if validate_launcher_structure(&settings.launcher.items).is_err() {
                settings.launcher.items.clear();
                changed = true;
            }
        } else {
            changed = true;
        }
    }
    let window = fields(root.get("window"), &["position"], &mut changed);
    let position = fields(window.get("position"), &["x", "y"], &mut changed);
    unknown |= window.unknown || position.unknown;
    match (
        scalar(position.get("x"), 64, &mut changed),
        scalar(position.get("y"), 64, &mut changed),
    ) {
        (Some(x), Some(y)) => settings.window.position = Some(WindowPosition { x, y }),
        _ if window
            .get("position")
            .is_some_and(|raw| raw.get() != "null") =>
        {
            changed = true
        }
        _ => {}
    }
    super::validation::validate_settings(&settings).map_err(|error| error.to_string())?;
    Ok(DecodedSettings {
        settings,
        needs_resave: changed && !unknown,
        read_only: unknown,
        unsupported_version: false,
    })
}

#[cfg(test)]
mod tests;
