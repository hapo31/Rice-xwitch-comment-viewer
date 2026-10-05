use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::Path;

pub(super) const MAX_LAUNCHER_ITEMS: usize = 200;
pub(super) const MAX_PATH_BYTES: usize = 4096;
pub(super) const MAX_PATHS_BYTES: usize = 128 * 1024;
pub(super) const MAX_ADD_REQUEST_BYTES: usize = 256 * 1024;
pub(super) const MAX_ID_BYTES: usize = 64;
pub(super) const MAX_DISPLAY_NAME_CHARS: usize = 120;
pub(super) const MAX_GROUP_CHARS: usize = 64;
pub(super) const MAX_TOTAL_ICON_BYTES: usize = 4 * 1024 * 1024;
pub(super) const UNSUPPORTED_LAUNCHER_MESSAGE: &str = "アプリの登録・起動はWindows版でのみ利用できます。このOSでは標準のランチャーから起動してください。保存済み項目の表示・削除はできます。";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct LauncherCapabilities {
    pub can_register_applications: bool,
    pub can_launch_applications: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl LauncherCapabilities {
    pub fn current() -> Self {
        Self::for_platform(cfg!(target_os = "windows"))
    }

    pub(super) fn for_platform(windows: bool) -> Self {
        Self {
            can_register_applications: windows,
            can_launch_applications: windows,
            reason: (!windows).then(|| UNSUPPORTED_LAUNCHER_MESSAGE.to_string()),
        }
    }

    pub(super) fn ensure_supported(&self) -> Result<(), String> {
        if self.can_register_applications && self.can_launch_applications {
            Ok(())
        } else {
            Err(UNSUPPORTED_LAUNCHER_MESSAGE.to_string())
        }
    }
}

pub(super) const LAUNCHER_ICON_DATA_URL_PREFIX: &str = "data:image/png;base64,";
pub(super) const MAX_ICON_BASE64_LENGTH: usize = 64 * 1024;
pub(super) const MAX_ICON_FILE_BYTES: usize = 48 * 1024;
pub(super) const MAX_ICON_DIMENSION: u32 = 128;
pub(super) const MAX_ICON_DECODED_BYTES: usize = 128 * 1024;
pub(super) const MAX_PNG_DECODER_BYTES: usize = 1024 * 1024;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct LauncherSettings {
    #[serde(default, deserialize_with = "deserialize_launcher_items")]
    pub items: Vec<LauncherItem>,
}

fn deserialize_launcher_items<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<LauncherItem>, D::Error> {
    struct Items;
    impl<'de> serde::de::Visitor<'de> for Items {
        type Value = Vec<LauncherItem>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("at most 200 launcher items")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> Result<Self::Value, A::Error> {
            let mut items =
                Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(MAX_LAUNCHER_ITEMS));
            for _ in 0..MAX_LAUNCHER_ITEMS {
                match sequence.next_element()? {
                    Some(item) => items.push(item),
                    None => return Ok(items),
                }
            }
            if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom(item_limit_message()));
            }
            Ok(items)
        }
    }
    deserializer.deserialize_seq(Items)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct LauncherItem {
    pub id: String,
    pub kind: LauncherItemKind,
    pub target: String,
    pub display_name: String,
    #[serde(
        default,
        deserialize_with = "deserialize_launcher_icon_data_url",
        skip_serializing_if = "Option::is_none"
    )]
    pub icon_data_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(default)]
    pub order: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum LauncherItemKind {
    Application,
    Website,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LauncherSettingsPatch {
    pub items: Option<Vec<LauncherItemEdit>>,
}

/// IDs select existing backend-owned items. Target/kind/icon are never editable
/// through settings_update, even on Windows. Registration uses launcher_add.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LauncherItemEdit {
    pub id: String,
    pub display_name: String,
    pub background_color: Option<String>,
    pub group_id: Option<String>,
    pub order: u32,
}

pub(crate) fn preflight_launcher_patch(value: &serde_json::Value) -> Result<(), String> {
    let Some(launcher) = value.get("launcher") else {
        return Ok(());
    };
    let launcher = launcher
        .as_object()
        .ok_or_else(|| "ランチャーの設定形式を確認してください。".to_string())?;
    if launcher.keys().any(|key| key != "items") {
        return Err("ランチャーの編集項目が無効です。登録先・アイコンは変更できません。".into());
    }
    let Some(items) = launcher.get("items") else {
        return Ok(());
    };
    let items = items
        .as_array()
        .ok_or_else(|| "ランチャー項目を一覧で指定してください。".to_string())?;
    if items.len() > MAX_LAUNCHER_ITEMS {
        return Err(item_limit_message());
    }
    for item in items {
        let fields = item
            .as_object()
            .ok_or_else(|| "ランチャー項目の形式を確認してください。".to_string())?;
        if fields.keys().any(|key| {
            !matches!(
                key.as_str(),
                "id" | "displayName" | "backgroundColor" | "groupId" | "order"
            )
        }) {
            return Err(
                "ランチャーの編集項目が無効です。登録先・アイコンは変更できません。".into(),
            );
        }
        validate_id(
            fields
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "登録済みのIDを指定してください。".to_string())?,
        )?;
        for (key, maximum) in [
            ("displayName", MAX_DISPLAY_NAME_CHARS),
            ("groupId", MAX_GROUP_CHARS),
        ] {
            if let Some(value) = fields.get(key) {
                if value.is_null() && key == "groupId" {
                    continue;
                }
                let text = value
                    .as_str()
                    .ok_or_else(|| "表示名・グループ名は文字列で指定してください。".to_string())?;
                if text.len() > maximum * 4
                    || text.chars().count() > maximum
                    || text.chars().any(char::is_control)
                {
                    return Err(format!(
                        "表示名は120文字、グループ名は64文字までです。{key}を短くしてください。"
                    ));
                }
            }
        }
        if fields.get("backgroundColor").is_some_and(|value| {
            !value.is_null()
                && value.as_str().is_none_or(|color| {
                    color.len() != 7
                        || !color.starts_with('#')
                        || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
                })
        }) {
            return Err("背景色は #RRGGBB 形式で指定してください。".into());
        }
    }
    Ok(())
}

pub(crate) fn apply_launcher_edits(
    existing: &[LauncherItem],
    edits: Vec<LauncherItemEdit>,
) -> Result<Vec<LauncherItem>, String> {
    if edits.len() > MAX_LAUNCHER_ITEMS {
        return Err(item_limit_message());
    }
    let mut result = Vec::with_capacity(edits.len());
    for edit in edits {
        validate_id(&edit.id)?;
        let mut item = existing
            .iter()
            .find(|item| item.id == edit.id)
            .cloned()
            .ok_or_else(|| {
                "登録済みのアプリだけを編集できます。新しいアプリは追加操作から登録してください。"
                    .to_string()
            })?;
        item.display_name = edit.display_name;
        item.background_color = edit.background_color;
        item.group_id = edit.group_id;
        item.order = edit.order;
        result.push(item);
    }
    normalize_launcher_items(result)
}

pub(super) fn item_limit_message() -> String {
    format!("ランチャーに登録できるアプリは最大 {MAX_LAUNCHER_ITEMS} 件です。不要な項目を削除してください。")
}

pub(super) fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > MAX_ID_BYTES
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(
            "ランチャー項目のIDは64バイト以内の英数字・ハイフン・下線にしてください。".into(),
        );
    }
    Ok(())
}

pub(super) fn validate_target_text(target: &str) -> Result<(), String> {
    if target.is_empty() || target.len() > MAX_PATH_BYTES || target.chars().any(char::is_control) {
        return Err(
            "アプリのパスは制御文字を含まない4096バイト以内の文字列にしてください。".into(),
        );
    }
    if !is_supported_application_path(Path::new(target)) {
        return Err("追加できるのは .exe または .lnk ファイルだけです。".into());
    }
    Ok(())
}

pub(super) fn validate_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
    let mut count = 0;
    let mut total = 0usize;
    for path in paths {
        count += 1;
        if count > MAX_LAUNCHER_ITEMS {
            return Err(item_limit_message());
        }
        if path.len() > MAX_PATH_BYTES || path.chars().any(char::is_control) {
            return Err(
                "アプリのパスは制御文字を含まない4096バイト以内に短くしてください。".into(),
            );
        }
        validate_target_text(path.trim())?;
        // Include whitespace and duplicates: neither may bypass the wire budget.
        total = total
            .checked_add(path.len())
            .ok_or_else(|| "パスの合計量が大きすぎます。".to_string())?;
        if total > MAX_PATHS_BYTES {
            return Err(
                "パスの合計は128KiBまでです。一度に追加する項目数やパスを短くしてください。".into(),
            );
        }
    }
    if count == 0 {
        return Err("追加するアプリを選択してください。".into());
    }
    Ok(())
}

pub(super) fn parse_add_request(value: &serde_json::Value) -> Result<Vec<String>, String> {
    crate::resource_limits::validate_json_request(value, MAX_ADD_REQUEST_BYTES)?;
    let paths = value
        .get("paths")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "追加するアプリのパス一覧を指定してください。".to_string())?;
    if paths.len() > MAX_LAUNCHER_ITEMS {
        return Err(item_limit_message());
    }
    let borrowed = paths
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "アプリのパスは文字列で指定してください。".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_paths(borrowed.iter().copied())?;
    Ok(borrowed.into_iter().map(str::to_string).collect())
}

pub(crate) fn validate_launcher_resources(items: &[LauncherItem]) -> Result<(), String> {
    validate_launcher_structure(items)?;
    // Check the aggregate before decoding any PNG. Only bounded buffers are used.
    for item in items {
        if item
            .icon_data_url
            .as_ref()
            .is_some_and(|icon| !valid_icon_data_url(icon))
        {
            return Err("アイコンは正しいPNG画像48KiB・128×128以内にしてください。".into());
        }
    }
    Ok(())
}

/// Deserialized items already passed the full PNG decoder individually. Avoid
/// decoding them a second time on file load; still enforce the shared quotas.
pub(crate) fn validate_launcher_structure(items: &[LauncherItem]) -> Result<(), String> {
    if items.len() > MAX_LAUNCHER_ITEMS {
        return Err(item_limit_message());
    }
    let mut ids = HashSet::with_capacity(items.len());
    let mut targets = HashSet::with_capacity(items.len());
    let mut icon_bytes = 0usize;
    let mut path_bytes = 0usize;
    for item in items {
        if item.kind != LauncherItemKind::Application {
            return Err("Webサイトのリンクはまだ登録できません。".into());
        }
        validate_id(&item.id)?;
        validate_target_text(&item.target)?;
        path_bytes += item.target.len();
        if path_bytes > MAX_PATHS_BYTES {
            return Err(
                "登録済みパスの合計は128KiBまでです。項目数やパスを短くしてください。".into(),
            );
        }
        if !ids.insert(&item.id) {
            return Err(
                "ランチャー項目のIDが重複しています。重複した項目を削除してください。".into(),
            );
        }
        if !targets.insert(path_identity_key(Path::new(&item.target))) {
            return Err(
                "同じアプリが複数登録されています。重複した項目を削除してください。".into(),
            );
        }
        if item.display_name.is_empty()
            || item.display_name.len() > MAX_DISPLAY_NAME_CHARS * 4
            || item.display_name.chars().count() > MAX_DISPLAY_NAME_CHARS
            || item.display_name.chars().any(char::is_control)
        {
            return Err("アプリの表示名は制御文字を含まない1〜120文字にしてください。".into());
        }
        if let Some(group) = &item.group_id {
            if group.is_empty()
                || group.len() > MAX_GROUP_CHARS * 4
                || group.chars().count() > MAX_GROUP_CHARS
                || group.chars().any(char::is_control)
            {
                return Err("グループ名は制御文字を含まない1〜64文字にしてください。".into());
            }
        }
        if let Some(color) = &item.background_color {
            if color.len() != 7
                || !color.starts_with('#')
                || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
            {
                return Err("背景色は #RRGGBB 形式で指定してください。".into());
            }
        }
        if let Some(icon) = &item.icon_data_url {
            if icon.len() > MAX_ICON_BASE64_LENGTH + LAUNCHER_ICON_DATA_URL_PREFIX.len() {
                return Err("アイコンはPNG画像48KiB・128×128以内にしてください。".into());
            }
            icon_bytes += icon.len();
            if icon_bytes > MAX_TOTAL_ICON_BYTES {
                return Err(
                    "アイコンの合計は4MiBまでです。登録するアプリを減らしてください。".into(),
                );
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct LauncherAddResult {
    pub items: Vec<LauncherItem>,
    pub added_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct LauncherLaunchFailure {
    pub item_id: String,
    pub display_name: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
pub struct LauncherLaunchResult {
    /// Verified target process creation only; not shell acceptance/app readiness.
    pub launched_count: usize,
    pub failures: Vec<LauncherLaunchFailure>,
}

pub(crate) fn normalize_launcher_items(
    items: Vec<LauncherItem>,
) -> Result<Vec<LauncherItem>, String> {
    if items.len() > MAX_LAUNCHER_ITEMS {
        return Err(item_limit_message());
    }
    let mut normalized = items;
    for item in &mut normalized {
        // Stored, backend-owned paths must not touch disk during metadata edits
        // or load. Missing/moved files are diagnosed only by registration/launch.
        item.id = item.id.trim().to_string();
        item.target = item.target.trim().to_string();
        item.display_name = item.display_name.trim().to_string();
        item.background_color = normalize_optional_text(item.background_color.take());
        item.group_id = normalize_optional_text(item.group_id.take());
    }
    validate_launcher_resources(&normalized)?;
    Ok(normalized)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn deserialize_launcher_icon_data_url<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(normalize_launcher_icon_data_url)
}

pub(crate) fn normalize_launcher_icon_data_url(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if !valid_icon_data_url(trimmed) {
        return None;
    }
    if trimmed.len() == value.len() {
        Some(value)
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) fn valid_icon_data_url(value: &str) -> bool {
    let Some(encoded) = value.strip_prefix(LAUNCHER_ICON_DATA_URL_PREFIX) else {
        return false;
    };
    if encoded.is_empty() || encoded.len() > MAX_ICON_BASE64_LENGTH || encoded.len() % 4 != 0 {
        return false;
    }
    let Ok(decoded) = BASE64_STANDARD.decode(encoded) else {
        return false;
    };
    !decoded.is_empty() && decoded.len() <= MAX_ICON_FILE_BYTES && is_valid_launcher_png(&decoded)
}

fn is_valid_launcher_png(bytes: &[u8]) -> bool {
    let mut options = png::DecodeOptions::default();
    options.set_ignore_adler32(false);
    options.set_ignore_crc(false);
    options.set_ignore_text_chunk(true);
    options.set_ignore_iccp_chunk(true);
    options.set_skip_ancillary_crc_failures(false);

    let mut decoder = png::Decoder::new_with_options(Cursor::new(bytes), options);
    decoder.set_limits(png::Limits {
        bytes: MAX_PNG_DECODER_BYTES,
    });
    let mut reader = match decoder.read_info() {
        Ok(reader) => reader,
        Err(_) => return false,
    };
    let info = reader.info();
    if info.width == 0
        || info.height == 0
        || info.width > MAX_ICON_DIMENSION
        || info.height > MAX_ICON_DIMENSION
        || info.animation_control.is_some()
    {
        return false;
    }

    let output_size = reader.output_buffer_size();
    if output_size == 0 || output_size > MAX_ICON_DECODED_BYTES {
        return false;
    }
    let mut output = vec![0; output_size];
    reader.next_frame(&mut output).is_ok() && reader.finish().is_ok()
}

pub(super) fn derive_display_name(target: &Path) -> String {
    target
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .or_else(|| target.file_name().and_then(|name| name.to_str()))
        .unwrap_or("アプリ")
        .trim()
        .chars()
        .take(120)
        .collect()
}

pub(super) fn is_supported_application_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("lnk")
        })
}

pub(super) fn path_identity_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

pub(super) fn next_order(items: &[LauncherItem]) -> u32 {
    items
        .iter()
        .map(|item| item.order)
        .max()
        .map_or(0, |order| order.saturating_add(1))
}

pub(super) fn make_item_id(target: &Path, existing_ids: &HashSet<String>) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path_identity_key(target).hash(&mut hasher);
    let base = format!("launcher-{:016x}", hasher.finish());
    if !existing_ids.contains(&base) {
        return base;
    }

    (2_u32..)
        .map(|suffix| format!("{base}-{suffix}"))
        .find(|candidate| !existing_ids.contains(candidate))
        .expect("launcher item id space exhausted")
}
