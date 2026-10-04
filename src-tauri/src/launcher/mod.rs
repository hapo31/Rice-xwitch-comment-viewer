#[cfg(feature = "app")]
use crate::app_events::{emit_app_log, AppLogLevel};
use crate::settings::AppSettings;
#[cfg(feature = "app")]
use crate::settings::{update_settings_transaction, AppState, SettingsStore};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use futures_util::{stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
#[cfg(any(all(feature = "app", target_os = "windows"), test))]
use std::io::Read;
use std::path::{Path, PathBuf};
#[cfg(any(all(feature = "app", target_os = "windows"), test))]
use std::process::{Child, ExitStatus};
#[cfg(all(feature = "app", target_os = "windows"))]
use std::process::{Command, Stdio};
use std::sync::Arc;
#[cfg(feature = "app")]
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

const MAX_LAUNCHER_ITEMS: usize = 200;
const MAX_PATH_BYTES: usize = 4096;
const MAX_PATHS_BYTES: usize = 128 * 1024;
const MAX_ADD_REQUEST_BYTES: usize = 256 * 1024;
const MAX_ID_BYTES: usize = 64;
const MAX_DISPLAY_NAME_CHARS: usize = 120;
const MAX_GROUP_CHARS: usize = 64;
const MAX_TOTAL_ICON_BYTES: usize = 4 * 1024 * 1024;
const UNSUPPORTED_LAUNCHER_MESSAGE: &str = "アプリの登録・起動はWindows版でのみ利用できます。このOSでは標準のランチャーから起動してください。保存済み項目の表示・削除はできます。";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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

    fn for_platform(windows: bool) -> Self {
        Self {
            can_register_applications: windows,
            can_launch_applications: windows,
            reason: (!windows).then(|| UNSUPPORTED_LAUNCHER_MESSAGE.to_string()),
        }
    }

    fn ensure_supported(&self) -> Result<(), String> {
        if self.can_register_applications && self.can_launch_applications {
            Ok(())
        } else {
            Err(UNSUPPORTED_LAUNCHER_MESSAGE.to_string())
        }
    }
}

const LAUNCHER_ICON_DATA_URL_PREFIX: &str = "data:image/png;base64,";
const MAX_ICON_BASE64_LENGTH: usize = 64 * 1024;
const MAX_ICON_FILE_BYTES: usize = 48 * 1024;
const MAX_ICON_DIMENSION: u32 = 128;
const MAX_ICON_DECODED_BYTES: usize = 128 * 1024;
const MAX_PNG_DECODER_BYTES: usize = 1024 * 1024;
#[cfg(feature = "app")]
const MAX_LAUNCHER_ICON_WORKERS: usize = 4;
#[cfg(all(feature = "app", target_os = "windows"))]
const ICON_EXTRACTION_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(feature = "app")]
const LAUNCHER_WORKER_WAIT_TIMEOUT: Duration = Duration::from_secs(6);
#[cfg(feature = "app")]
const LAUNCHER_WORKER_JOB_TIMEOUT: Duration = Duration::from_secs(7);

// A timed-out blocking filesystem operation cannot be cancelled safely. Keep its
// permit until its worker really exits so a stalled network path cannot grow the
// blocking pool without bound across repeated add requests.
#[cfg(feature = "app")]
static LAUNCHER_ICON_WORKERS: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(MAX_LAUNCHER_ICON_WORKERS)));

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

fn item_limit_message() -> String {
    format!("ランチャーに登録できるアプリは最大 {MAX_LAUNCHER_ITEMS} 件です。不要な項目を削除してください。")
}

fn validate_id(id: &str) -> Result<(), String> {
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

fn validate_target_text(target: &str) -> Result<(), String> {
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

fn validate_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
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

fn parse_add_request(value: &serde_json::Value) -> Result<Vec<String>, String> {
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
pub struct LauncherLaunchFailure {
    pub item_id: String,
    pub display_name: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LauncherLaunchResult {
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

fn normalize_launcher_icon_data_url(value: Option<String>) -> Option<String> {
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

fn valid_icon_data_url(value: &str) -> bool {
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

fn derive_display_name(target: &Path) -> String {
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

fn validate_application_target(raw_target: &str) -> Result<PathBuf, String> {
    validate_paths(std::iter::once(raw_target))?;
    let raw_target = raw_target.trim();
    if raw_target.is_empty() {
        return Err("アプリのパスが空です。".to_string());
    }

    let target = Path::new(raw_target);
    if !is_supported_application_path(target) {
        return Err(format!(
            "追加できるのは .exe または .lnk ファイルだけです: {}",
            target.display()
        ));
    }
    if !target.exists() {
        return Err(format!(
            "アプリが見つかりません。移動または削除されていないか確認してください: {}",
            target.display()
        ));
    }
    if !target.is_file() {
        return Err(format!(
            "アプリのファイルを指定してください: {}",
            target.display()
        ));
    }

    target
        .canonicalize()
        .map(normalize_canonical_path)
        .map_err(|error| {
            format!(
                "アプリのパスを確認できませんでした: {} ({error})",
                target.display()
            )
        })
}

#[cfg(target_os = "windows")]
fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    // std::fs::canonicalize uses the Win32 verbatim prefix. Explorer,
    // PowerShell's drawing APIs and some applications expect a regular DOS/UNC path.
    let path_text = path.to_string_lossy();
    if let Some(rest) = path_text.strip_prefix("\\\\?\\UNC\\") {
        return PathBuf::from(format!("\\\\{rest}"));
    }
    if let Some(rest) = path_text.strip_prefix("\\\\?\\") {
        return PathBuf::from(rest);
    }
    path
}

#[cfg(not(target_os = "windows"))]
fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    path
}

fn is_supported_application_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("lnk")
        })
}

fn path_identity_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn next_order(items: &[LauncherItem]) -> u32 {
    items
        .iter()
        .map(|item| item.order)
        .max()
        .map_or(0, |order| order.saturating_add(1))
}

fn make_item_id(target: &Path, existing_ids: &HashSet<String>) -> String {
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

#[derive(Debug, Clone)]
struct IconExtractionWarning {
    target: PathBuf,
    message: String,
    elapsed: Duration,
}

#[derive(Debug, Clone)]
struct PreparedLauncherItem {
    target: PathBuf,
    icon_data_url: Option<String>,
    icon_warning: Option<IconExtractionWarning>,
}

trait LauncherIconExtractor: Send + Sync {
    fn extract(&self, target: &Path) -> Result<Option<String>, IconExtractionError>;
}

#[derive(Debug, Clone)]
enum IconExtractionError {
    Failed(String),
    #[cfg(any(target_os = "windows", test))]
    ResourceLimit,
}
impl From<String> for IconExtractionError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

struct SystemIconExtractor;

impl LauncherIconExtractor for SystemIconExtractor {
    fn extract(&self, target: &Path) -> Result<Option<String>, IconExtractionError> {
        extract_icon_data_url(target)
    }
}

fn prepare_launcher_item(
    raw_target: String,
    extractor: &dyn LauncherIconExtractor,
) -> Result<PreparedLauncherItem, String> {
    let target = validate_application_target(&raw_target)?;
    let started_at = Instant::now();
    let (icon_data_url, icon_warning) = match extractor.extract(&target) {
        Ok(icon_data_url) => {
            if icon_data_url.as_ref().is_some_and(|icon| !valid_icon_data_url(icon)) { return Err("アイコンは正しいPNG画像48KiB・128×128以内にしてください。小さいアイコンのアプリを選んでください。追加内容は保存していません。".into()); }
            (icon_data_url, None)
        },
        #[cfg(any(target_os = "windows", test))]
        Err(IconExtractionError::ResourceLimit) => return Err("アイコン出力は64KiBまでです。小さいアイコンのアプリを選んでください。追加内容は保存していません。".into()),
        Err(IconExtractionError::Failed(message)) => (
            None,
            Some(IconExtractionWarning {
                target: target.clone(),
                message,
                elapsed: started_at.elapsed(),
            }),
        ),
    };

    Ok(PreparedLauncherItem {
        target,
        icon_data_url,
        icon_warning,
    })
}

#[derive(Debug, Clone)]
struct BuiltLauncherItems {
    items: Vec<LauncherItem>,
    icon_warnings: Vec<IconExtractionWarning>,
}

#[derive(Clone)]
struct LauncherWorkerConfig {
    worker_limit: usize,
    worker_pool: Arc<Semaphore>,
    acquire_timeout: Duration,
    job_timeout: Duration,
}

fn assemble_launcher_items(
    existing: &[LauncherItem],
    prepared: Vec<PreparedLauncherItem>,
) -> Result<BuiltLauncherItems, String> {
    let mut target_keys = existing
        .iter()
        .map(|item| path_identity_key(Path::new(&item.target)))
        .collect::<HashSet<_>>();
    let mut item_ids = existing
        .iter()
        .map(|item| item.id.clone())
        .collect::<HashSet<_>>();
    let mut order = next_order(existing);
    let mut new_items = Vec::with_capacity(prepared.len());
    let mut icon_warnings = Vec::new();

    for prepared in prepared {
        let target = prepared.target;
        if !target_keys.insert(path_identity_key(&target)) {
            continue;
        }
        if existing.len().saturating_add(new_items.len()) >= MAX_LAUNCHER_ITEMS {
            return Err(format!(
                "ランチャーに登録できるアプリは最大 {MAX_LAUNCHER_ITEMS} 件です。"
            ));
        }

        let id = make_item_id(&target, &item_ids);
        item_ids.insert(id.clone());
        new_items.push(LauncherItem {
            id,
            kind: LauncherItemKind::Application,
            target: target.to_string_lossy().into_owned(),
            display_name: derive_display_name(&target),
            icon_data_url: prepared.icon_data_url,
            background_color: None,
            group_id: None,
            order,
        });
        if let Some(warning) = prepared.icon_warning {
            icon_warnings.push(warning);
        }
        order = order.saturating_add(1);
    }

    Ok(BuiltLauncherItems {
        items: new_items,
        icon_warnings,
    })
}

async fn build_new_items_in_workers_with_extractor(
    existing: Vec<LauncherItem>,
    raw_targets: Vec<String>,
    extractor: Arc<dyn LauncherIconExtractor>,
    config: LauncherWorkerConfig,
) -> Result<BuiltLauncherItems, String> {
    validate_paths(raw_targets.iter().map(String::as_str))?;

    if config.worker_limit == 0 {
        return Err("ランチャーのアイコン確認 worker 数が無効です。".to_string());
    }

    let mut raw_keys = HashSet::new();
    let unique_targets = raw_targets
        .into_iter()
        .filter(|target| raw_keys.insert(path_identity_key(Path::new(target.trim()))));
    let prepared = stream::iter(unique_targets.enumerate())
        .map(|(index, raw_target)| {
            let config = config.clone();
            let extractor = Arc::clone(&extractor);
            async move {
            let permit = tokio::time::timeout(
                config.acquire_timeout,
                config.worker_pool.clone().acquire_owned(),
            )
            .await
            .map_err(|_| {
                "ランチャーのアイコン確認が混み合っています。しばらく待ってからもう一度追加してください。"
                    .to_string()
            })?
            .map_err(|_| "ランチャーのアイコン確認を開始できません。".to_string())?;

            let task = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                prepare_launcher_item(raw_target, extractor.as_ref())
            });
            let prepared = match tokio::time::timeout(config.job_timeout, task).await {
                Ok(Ok(prepared)) => prepared?,
                Ok(Err(error)) => {
                    return Err(format!("ランチャーのファイル確認に失敗しました: {error}"));
                }
                Err(_) => {
                    // A running blocking task cannot be force-cancelled, so it retains its
                    // semaphore permit until it returns. That cap is the recovery boundary for
                    // hung filesystem calls.
                    return Err(
                        "ランチャーのファイル確認がタイムアウトしました。ネットワーク上のショートカットを確認してください。"
                            .to_string(),
                    );
                }
            };
            Ok::<_, String>((index, prepared))
            }
        })
        .buffer_unordered(config.worker_limit)
        .try_collect::<Vec<_>>()
        .await;

    let mut prepared = prepared?;
    prepared.sort_by_key(|(index, _)| *index);
    assemble_launcher_items(
        &existing,
        prepared.into_iter().map(|(_, item)| item).collect(),
    )
}

#[cfg(feature = "app")]
async fn build_new_items_in_workers(
    existing: Vec<LauncherItem>,
    raw_targets: Vec<String>,
) -> Result<BuiltLauncherItems, String> {
    build_new_items_in_workers_with_extractor(
        existing,
        raw_targets,
        Arc::new(SystemIconExtractor),
        LauncherWorkerConfig {
            worker_limit: MAX_LAUNCHER_ICON_WORKERS,
            worker_pool: LAUNCHER_ICON_WORKERS.clone(),
            acquire_timeout: LAUNCHER_WORKER_WAIT_TIMEOUT,
            job_timeout: LAUNCHER_WORKER_JOB_TIMEOUT,
        },
    )
    .await
}

fn merge_new_launcher_items(
    existing: &[LauncherItem],
    new_items: Vec<LauncherItem>,
) -> Result<Vec<LauncherItem>, String> {
    let mut target_keys = existing
        .iter()
        .map(|item| path_identity_key(Path::new(&item.target)))
        .collect::<HashSet<_>>();
    let mut item_ids = existing
        .iter()
        .map(|item| item.id.clone())
        .collect::<HashSet<_>>();
    let mut order = next_order(existing);
    let mut merged = Vec::new();

    for mut item in new_items {
        let target = Path::new(&item.target);
        if !target_keys.insert(path_identity_key(target)) {
            continue;
        }
        if existing.len().saturating_add(merged.len()) >= MAX_LAUNCHER_ITEMS {
            return Err(item_limit_message());
        }
        item.id = make_item_id(target, &item_ids);
        item_ids.insert(item.id.clone());
        item.order = order;
        order = order.saturating_add(1);
        merged.push(item);
    }
    // Validate the entire candidate, not just the additions, before any save.
    let mut candidate = existing.to_vec();
    candidate.extend(merged.iter().cloned());
    validate_launcher_resources(&candidate)?;
    Ok(merged)
}

fn launcher_items_snapshot(
    settings: &std::sync::Mutex<AppSettings>,
) -> Result<Vec<LauncherItem>, String> {
    settings
        .lock()
        .map_err(|error| error.to_string())
        .map(|settings| settings.launcher.items.clone())
}

#[cfg(any(all(feature = "app", target_os = "windows"), test))]
enum ChildExitWaitError {
    Wait(std::io::Error),
    TimedOut {
        termination: Result<ExitStatus, String>,
    },
}

#[cfg(any(all(feature = "app", target_os = "windows"), test))]
fn wait_for_child_exit(
    child: &mut Child,
    timeout: Duration,
) -> Result<ExitStatus, ChildExitWaitError> {
    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started_at.elapsed() >= timeout => {
                return Err(ChildExitWaitError::TimedOut {
                    termination: terminate_and_reap_child(child),
                });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => return Err(ChildExitWaitError::Wait(error)),
        }
    }
}

#[cfg(any(all(feature = "app", target_os = "windows"), test))]
fn terminate_and_reap_child(child: &mut Child) -> Result<ExitStatus, String> {
    match child.kill() {
        Ok(()) => child
            .wait()
            .map_err(|error| format!("終了を待機できませんでした: {error}")),
        Err(kill_error) => match child.try_wait() {
            Ok(Some(status)) => Ok(status),
            Ok(None) => Err(format!("終了要求を送れませんでした: {kill_error}")),
            Err(wait_error) => Err(format!(
                "終了要求を送れず、状態も確認できませんでした: {kill_error}; {wait_error}"
            )),
        },
    }
}

#[cfg(all(feature = "app", target_os = "windows"))]
fn extract_icon_data_url(target: &Path) -> Result<Option<String>, IconExtractionError> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const EXTRACT_ICON_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$path = $env:RICE_LAUNCHER_ICON_PATH
$source = $path
if ([IO.Path]::GetExtension($path) -ieq '.lnk') {
  $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($path)
  $iconLocation = ($shortcut.IconLocation -split ',')[0].Trim('"')
  if ($iconLocation -and [IO.File]::Exists($iconLocation)) {
    $source = $iconLocation
  } elseif ($shortcut.TargetPath -and [IO.File]::Exists($shortcut.TargetPath)) {
    $source = $shortcut.TargetPath
  }
}
$icon = [Drawing.Icon]::ExtractAssociatedIcon($source)
if ($null -eq $icon) { exit 2 }
$bitmap = $icon.ToBitmap()
$stream = New-Object IO.MemoryStream
try {
  $bitmap.Save($stream, [Drawing.Imaging.ImageFormat]::Png)
  [Console]::Out.Write([Convert]::ToBase64String($stream.ToArray()))
} finally {
  $stream.Dispose()
  $bitmap.Dispose()
  $icon.Dispose()
}
"#;

    let mut child = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            EXTRACT_ICON_SCRIPT,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .env("RICE_LAUNCHER_ICON_PATH", target.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("PowerShell を開始できませんでした: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "PowerShell の出力を取得できませんでした。".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "PowerShell のエラー出力を取得できませんでした。".to_string())?;
    let stdout_reader =
        std::thread::spawn(move || read_pipe_bounded(stdout, MAX_ICON_BASE64_LENGTH));
    let stderr_reader = std::thread::spawn(move || read_pipe_bounded(stderr, 8 * 1024));
    let status = match wait_for_child_exit(&mut child, ICON_EXTRACTION_TIMEOUT) {
        Ok(status) => status,
        Err(ChildExitWaitError::TimedOut { termination }) => {
            return match termination {
                Ok(status) => {
                    let stdout_result = stdout_reader.join();
                    let stderr_result = stderr_reader.join();
                    let pipe_error = !matches!(stdout_result, Ok(Ok(_)))
                        || !matches!(stderr_result, Ok(Ok(_)));
                    if pipe_error {
                        Err(format!(
                            "PowerShell のアイコン抽出が {} 秒でタイムアウトしました。子プロセスは終了しました（{status}）が、出力回収を確認できませんでした。",
                            ICON_EXTRACTION_TIMEOUT.as_secs()
                        ))
                    } else {
                        Err(format!(
                            "PowerShell のアイコン抽出が {} 秒でタイムアウトしました。子プロセスの終了を確認しました（{status}）。",
                            ICON_EXTRACTION_TIMEOUT.as_secs()
                        ))
                    }
                }
                Err(error) => Err(format!(
                    "PowerShell のアイコン抽出が {} 秒でタイムアウトしました。子プロセスの終了を確認できませんでした: {error}",
                    ICON_EXTRACTION_TIMEOUT.as_secs()
                )),
            }.map_err(IconExtractionError::from);
        }
        Err(ChildExitWaitError::Wait(error)) => {
            let termination = terminate_and_reap_child(&mut child);
            if termination.is_ok() {
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
            }
            return Err(IconExtractionError::Failed(match termination {
                Ok(status) => format!(
                    "PowerShell の状態を確認できませんでした。子プロセスの終了を確認しました（{status}）: {error}"
                ),
                Err(termination_error) => format!(
                    "PowerShell の状態を確認できませんでした。子プロセスの終了も確認できませんでした: {error}; {termination_error}"
                ),
            }));
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "PowerShell の出力処理が停止しました。".to_string())?
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::InvalidData {
                IconExtractionError::ResourceLimit
            } else {
                IconExtractionError::Failed(format!("PowerShell の出力を読めませんでした: {error}"))
            }
        })?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "PowerShell のエラー出力処理が停止しました。".to_string())?
        .map_err(|error| format!("PowerShell のエラー出力を読めませんでした: {error}"))?;
    if !status.success() {
        let details = String::from_utf8_lossy(&stderr)
            .trim()
            .chars()
            .take(400)
            .collect::<String>();
        return Err(IconExtractionError::Failed(if details.is_empty() {
            format!("PowerShell のアイコン抽出が終了コード {status} で失敗しました。")
        } else {
            format!("PowerShell のアイコン抽出に失敗しました: {details}")
        }));
    }

    let encoded = String::from_utf8(stdout)
        .map_err(|_| "PowerShell のアイコン出力が文字列ではありません。".to_string())?;
    let encoded = encoded.trim();
    if encoded.is_empty() || encoded.len() > MAX_ICON_BASE64_LENGTH {
        return Err(IconExtractionError::ResourceLimit);
    }
    Ok(Some(format!("{LAUNCHER_ICON_DATA_URL_PREFIX}{encoded}")))
}

#[cfg(any(not(feature = "app"), not(target_os = "windows")))]
fn extract_icon_data_url(_target: &Path) -> Result<Option<String>, IconExtractionError> {
    Ok(None)
}

#[cfg(any(all(feature = "app", target_os = "windows"), test))]
fn read_pipe_bounded(mut pipe: impl Read, maximum: usize) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::with_capacity(maximum.min(8 * 1024));
    let mut buffer = [0_u8; 4096];
    let mut exceeded = false;
    loop {
        let read = pipe.read(&mut buffer)?;
        if read == 0 {
            if exceeded {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "icon extraction output exceeds limit",
                ));
            }
            return Ok(output);
        }
        let remaining = maximum.saturating_sub(output.len());
        exceeded |= read > remaining;
        output.extend_from_slice(&buffer[..read.min(remaining)]);
    }
}

#[cfg(feature = "app")]
#[tauri::command]
pub async fn launcher_add(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<Vec<LauncherItem>, String> {
    let paths = parse_add_request(crate::resource_limits::request_json(&request)?)?;
    LauncherCapabilities::current().ensure_supported()?;
    // Snapshot only: filesystem and COM work must never run while the settings
    // mutex is held, because that mutex is also used by chat and speech commands.
    let existing = launcher_items_snapshot(&state.settings)?;
    let BuiltLauncherItems {
        items: built_items,
        icon_warnings,
    } = build_new_items_in_workers(existing, paths).await?;

    // Settings can change while workers run. Merge validated targets against the
    // latest state so an overlapping add does not overwrite a newer save.
    let (items, added_count) = {
        let mut settings = state.settings.lock().map_err(|error| error.to_string())?;
        let new_items = merge_new_launcher_items(&settings.launcher.items, built_items)?;
        let added_count = new_items.len();
        update_settings_transaction(
            &mut settings,
            |candidate| {
                candidate.launcher.items.extend(new_items);
                Ok(())
            },
            |candidate| {
                SettingsStore::save(&app, candidate)
                    .map_err(|error| format!("ランチャーの設定を保存できませんでした: {error}"))
            },
        )?;
        (settings.launcher.items.clone(), added_count)
    };
    emit_app_log(
        &app,
        AppLogLevel::Info,
        format!("ランチャーにアプリを {added_count} 件追加しました。"),
    );
    emit_icon_extraction_warnings(&app, &icon_warnings);
    Ok(items)
}

#[cfg(feature = "app")]
fn emit_icon_extraction_warnings(
    app: &tauri::AppHandle<tauri::Wry>,
    warnings: &[IconExtractionWarning],
) {
    const MAX_LOGGED_WARNINGS: usize = 3;

    for warning in warnings.iter().take(MAX_LOGGED_WARNINGS) {
        emit_app_log(
            app,
            AppLogLevel::Warning,
            format!(
                "ランチャーのアイコンを取得できなかったため汎用アイコンを使います: {}（{}、{} ms）",
                warning.target.display(),
                warning.message,
                warning.elapsed.as_millis()
            ),
        );
    }
    if warnings.len() > MAX_LOGGED_WARNINGS {
        emit_app_log(
            app,
            AppLogLevel::Warning,
            format!(
                "ランチャーのアイコン取得失敗がさらに {} 件あります。ログは最大 {MAX_LOGGED_WARNINGS} 件まで表示します。",
                warnings.len() - MAX_LOGGED_WARNINGS
            ),
        );
    }
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn launcher_remove(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
    item_id: String,
) -> Result<Vec<LauncherItem>, String> {
    let item_id = item_id.trim();
    let mut settings = state.settings.lock().map_err(|error| error.to_string())?;
    let Some(index) = settings
        .launcher
        .items
        .iter()
        .position(|item| item.id == item_id)
    else {
        return Err("削除するランチャー項目が見つかりません。".to_string());
    };

    let removed = settings.launcher.items[index].clone();
    update_settings_transaction(
        &mut settings,
        |candidate| {
            candidate.launcher.items.remove(index);
            Ok(())
        },
        |candidate| {
            SettingsStore::save(&app, candidate)
                .map_err(|error| format!("ランチャーの設定を保存できませんでした: {error}"))
        },
    )?;

    let items = settings.launcher.items.clone();
    drop(settings);
    emit_app_log(
        &app,
        AppLogLevel::Info,
        format!("ランチャーから「{}」を削除しました。", removed.display_name),
    );
    Ok(items)
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn launcher_launch(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
    item_id: String,
) -> LauncherLaunchResult {
    let item = match state.settings.lock() {
        Ok(settings) => settings
            .launcher
            .items
            .iter()
            .find(|item| item.id == item_id.trim())
            .cloned(),
        Err(error) => {
            return LauncherLaunchResult {
                launched_count: 0,
                failures: vec![LauncherLaunchFailure {
                    item_id,
                    display_name: "アプリ".to_string(),
                    message: format!("ランチャーの設定を読み込めませんでした: {error}"),
                }],
            };
        }
    };

    let Some(item) = item else {
        return LauncherLaunchResult {
            launched_count: 0,
            failures: vec![LauncherLaunchFailure {
                item_id,
                display_name: "アプリ".to_string(),
                message: "起動するランチャー項目が見つかりません。".to_string(),
            }],
        };
    };

    let result = launch_items(std::slice::from_ref(&item));
    log_launch_result(&app, &result);
    result
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn launcher_launch_all(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
) -> LauncherLaunchResult {
    let mut items = match state.settings.lock() {
        Ok(settings) => settings.launcher.items.clone(),
        Err(error) => {
            return LauncherLaunchResult {
                launched_count: 0,
                failures: vec![LauncherLaunchFailure {
                    item_id: String::new(),
                    display_name: "ランチャー".to_string(),
                    message: format!("ランチャーの設定を読み込めませんでした: {error}"),
                }],
            };
        }
    };
    items.sort_by_key(|item| item.order);

    let result = launch_items(&items);
    log_launch_result(&app, &result);
    result
}

#[cfg(feature = "app")]
fn launch_items(items: &[LauncherItem]) -> LauncherLaunchResult {
    let mut result = LauncherLaunchResult::default();
    for item in items {
        match launch_item(item) {
            Ok(()) => result.launched_count += 1,
            Err(message) => result.failures.push(LauncherLaunchFailure {
                item_id: item.id.clone(),
                display_name: item.display_name.clone(),
                message,
            }),
        }
    }
    result
}

#[cfg(feature = "app")]
fn launch_item(item: &LauncherItem) -> Result<(), String> {
    LauncherCapabilities::current().ensure_supported()?;
    if item.kind != LauncherItemKind::Application {
        return Err("この種類のランチャー項目はまだ起動できません。".to_string());
    }

    let target = validate_application_target(&item.target)?;
    spawn_application(&target).map_err(|error| {
        format!("アプリを起動できませんでした。ファイルの場所や実行権限を確認してください: {error}")
    })
}

#[cfg(all(feature = "app", target_os = "windows"))]
fn spawn_application(target: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;

    // CREATE_NO_WINDOW prevents console applications from flashing a console window.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let is_shortcut = target
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"));
    let mut command = if is_shortcut {
        let mut command = Command::new("explorer.exe");
        command.arg(target);
        command
    } else {
        // The target is passed directly to CreateProcess without a command shell. No user
        // controlled text is interpreted as command-line arguments.
        let mut command = Command::new(target);
        if let Some(parent) = target.parent() {
            command.current_dir(parent);
        }
        command
    };

    command
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(all(feature = "app", not(target_os = "windows")))]
fn spawn_application(_target: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "アプリの起動は Windows でのみ利用できます。",
    ))
}

#[cfg(feature = "app")]
fn log_launch_result(app: &tauri::AppHandle<tauri::Wry>, result: &LauncherLaunchResult) {
    if result.failures.is_empty() {
        emit_app_log(
            app,
            AppLogLevel::Info,
            format!(
                "ランチャーからアプリを {} 件起動しました。",
                result.launched_count
            ),
        );
    } else {
        emit_app_log(
            app,
            AppLogLevel::Warning,
            format!(
                "ランチャーから {} 件起動し、{} 件は起動できませんでした。",
                result.launched_count,
                result.failures.len()
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    #[cfg(all(feature = "app", not(target_os = "windows")))]
    use super::launch_items;
    use super::LauncherCapabilities;
    use super::{
        build_new_items_in_workers_with_extractor, derive_display_name,
        is_supported_application_path, launcher_items_snapshot, merge_new_launcher_items,
        next_order, normalize_launcher_icon_data_url, normalize_launcher_items, path_identity_key,
        wait_for_child_exit, ChildExitWaitError, LauncherIconExtractor, LauncherItem,
        LauncherItemKind, LauncherWorkerConfig, SystemIconExtractor,
    };
    use crate::settings::AppSettings;
    use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tokio::sync::Semaphore;

    static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn launcher_platform_capabilities_allow_windows_and_reject_other_platforms() {
        let windows = LauncherCapabilities::for_platform(true);
        assert!(windows.can_register_applications && windows.can_launch_applications);
        assert!(windows.ensure_supported().is_ok());
        assert!(windows.reason.is_none());
        let unsupported = LauncherCapabilities::for_platform(false);
        assert!(!unsupported.can_register_applications && !unsupported.can_launch_applications);
        assert!(unsupported
            .ensure_supported()
            .unwrap_err()
            .contains("Windows版"));
        let json = serde_json::to_value(&unsupported).unwrap();
        assert_eq!(json["canRegisterApplications"], false);
        assert_eq!(json["canLaunchApplications"], false);
        assert!(json["reason"]
            .as_str()
            .unwrap()
            .contains("標準のランチャー"));
        assert_eq!(
            LauncherCapabilities::current().can_register_applications,
            cfg!(target_os = "windows")
        );
    }

    #[test]
    fn unsupported_platform_rejects_registration_and_target_changes_but_allows_removal() {
        let old = item(0);
        let edit = super::LauncherItemEdit {
            id: old.id.clone(),
            display_name: "表示名変更".into(),
            background_color: None,
            group_id: None,
            order: 0,
        };
        assert!(super::apply_launcher_edits(&[], vec![edit.clone()]).is_err());
        assert!(
            super::apply_launcher_edits(std::slice::from_ref(&old), vec![])
                .unwrap()
                .is_empty()
        );
        let changed = super::apply_launcher_edits(std::slice::from_ref(&old), vec![edit]).unwrap();
        assert_eq!(changed[0].target, old.target);
        assert_eq!(changed[0].display_name, "表示名変更");
        // Changing a target is forbidden on every OS, not only unsupported ones.
        assert!(super::preflight_launcher_patch(&serde_json::json!({"launcher": {"items": [{"id": old.id, "displayName": "name", "order": 0, "target": "C:\\moved.exe"}]}})).is_err());
    }

    #[cfg(not(target_os = "windows"))]
    #[cfg(feature = "app")]
    #[test]
    fn unsupported_launch_is_rejected_before_filesystem_access() {
        let result = launch_items(&[item(0)]);
        assert_eq!(result.launched_count, 0);
        assert_eq!(result.failures.len(), 1);
        assert!(result.failures[0].message.contains("Windows版"));
    }

    #[test]
    fn unsupported_registration_does_not_mutate_or_persist_settings() {
        let mut settings = AppSettings::default();
        let persisted = AtomicBool::new(false);
        let incoming = vec![item(0)];
        let result = crate::settings::update_settings_transaction(
            &mut settings,
            |candidate| {
                LauncherCapabilities::for_platform(false).ensure_supported()?;
                candidate.launcher.items = incoming;
                Ok(())
            },
            |_| {
                persisted.store(true, Ordering::SeqCst);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(settings.launcher.items.is_empty());
        assert!(!persisted.load(Ordering::SeqCst));
    }

    struct TemporaryFile(PathBuf);

    struct DelayedExtractor {
        result: Result<Option<String>, String>,
        delay: Duration,
        started: Option<Arc<AtomicBool>>,
        active: Option<Arc<AtomicUsize>>,
        peak_active: Option<Arc<AtomicUsize>>,
    }

    impl LauncherIconExtractor for DelayedExtractor {
        fn extract(&self, _target: &Path) -> Result<Option<String>, super::IconExtractionError> {
            let active = self
                .active
                .as_ref()
                .map(|active| active.fetch_add(1, Ordering::SeqCst) + 1);
            if let (Some(active), Some(peak_active)) = (active, &self.peak_active) {
                peak_active.fetch_max(active, Ordering::SeqCst);
            }
            if let Some(started) = &self.started {
                started.store(true, Ordering::SeqCst);
            }
            std::thread::sleep(self.delay);
            if let Some(active) = &self.active {
                active.fetch_sub(1, Ordering::SeqCst);
            }
            self.result.clone().map_err(Into::into)
        }
    }

    fn test_worker_config(worker_limit: usize, job_timeout: Duration) -> LauncherWorkerConfig {
        LauncherWorkerConfig {
            worker_limit,
            worker_pool: Arc::new(Semaphore::new(worker_limit)),
            acquire_timeout: Duration::from_secs(1),
            job_timeout,
        }
    }

    async fn build_for_test(
        existing: Vec<LauncherItem>,
        raw_targets: Vec<String>,
        extractor: Arc<dyn LauncherIconExtractor>,
        worker_limit: usize,
        job_timeout: Duration,
    ) -> Result<super::BuiltLauncherItems, String> {
        build_new_items_in_workers_with_extractor(
            existing,
            raw_targets,
            extractor,
            test_worker_config(worker_limit, job_timeout),
        )
        .await
    }

    impl TemporaryFile {
        fn application(extension: &str) -> Self {
            let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rice-launcher-test-{}-{sequence}.{extension}",
                std::process::id()
            ));
            fs::write(&path, b"launcher test").expect("create temporary application file");
            Self(path)
        }
    }

    impl Drop for TemporaryFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn item(order: u32) -> LauncherItem {
        LauncherItem {
            id: format!("item-{order}"),
            kind: LauncherItemKind::Application,
            target: format!(r"C:\Apps\app-{order}.exe"),
            display_name: format!("App {order}"),
            icon_data_url: None,
            background_color: None,
            group_id: None,
            order,
        }
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("write PNG header");
            writer
                .write_image_data(&vec![0; width as usize * height as usize * 4])
                .expect("write PNG pixels");
        }
        bytes
    }

    fn png_data_url(bytes: &[u8]) -> String {
        format!("data:image/png;base64,{}", BASE64_STANDARD.encode(bytes))
    }

    #[test]
    fn recognizes_supported_extensions_without_case_sensitivity() {
        assert!(is_supported_application_path(Path::new("app.exe")));
        assert!(is_supported_application_path(Path::new("APP.EXE")));
        assert!(is_supported_application_path(Path::new("shortcut.LnK")));
        assert!(!is_supported_application_path(Path::new("script.bat")));
        assert!(!is_supported_application_path(Path::new("app.exe.txt")));
    }

    #[test]
    fn launcher_icons_only_allow_base64_png_data_urls() {
        let valid = png_data_url(&png_bytes(1, 1));
        assert_eq!(
            normalize_launcher_icon_data_url(Some(format!(" {valid} "))),
            Some(valid)
        );
        assert_eq!(
            normalize_launcher_icon_data_url(Some("https://example.com/icon.png".to_string())),
            None
        );
        assert_eq!(
            normalize_launcher_icon_data_url(Some(
                "data:image/svg+xml;base64,PHN2Zz4=".to_string()
            )),
            None
        );
        assert_eq!(
            normalize_launcher_icon_data_url(Some(
                "data:image/png;base64,iVBORw0KGgo!".to_string()
            )),
            None
        );
        assert_eq!(
            normalize_launcher_icon_data_url(Some(
                "data:image/png;base64,iVBORw0KGgo=".to_string()
            )),
            None
        );
    }

    #[test]
    fn launcher_icons_reject_incomplete_or_corrupt_pngs() {
        let complete = png_bytes(1, 1);

        let without_iend = &complete[..complete.len() - 12];
        assert_eq!(
            normalize_launcher_icon_data_url(Some(png_data_url(without_iend))),
            None
        );

        let mut corrupt_ihdr = complete;
        corrupt_ihdr[16] ^= 1;
        assert_eq!(
            normalize_launcher_icon_data_url(Some(png_data_url(&corrupt_ihdr))),
            None
        );
    }

    #[test]
    fn launcher_icons_reject_dimensions_above_the_display_limit() {
        assert_eq!(
            normalize_launcher_icon_data_url(Some(png_data_url(&png_bytes(513, 1)))),
            None
        );
    }

    #[test]
    fn deserialization_drops_untrusted_launcher_icon_sources() {
        let item = serde_json::from_value::<LauncherItem>(serde_json::json!({
            "id": "item-1",
            "kind": "application",
            "target": "C:\\Apps\\app.exe",
            "displayName": "App",
            "iconDataUrl": "https://example.com/tracking.png",
            "order": 0
        }))
        .expect("deserialize launcher item");

        assert_eq!(item.icon_data_url, None);
    }

    #[test]
    fn derives_display_name_from_file_stem() {
        assert_eq!(
            derive_display_name(Path::new("/Apps/OBS Studio.exe")),
            "OBS Studio"
        );
        assert_eq!(
            derive_display_name(Path::new("配信ツール.lnk")),
            "配信ツール"
        );
    }

    #[test]
    fn path_identity_is_separator_and_case_insensitive() {
        assert_eq!(
            path_identity_key(Path::new(r"C:\Apps\OBS.EXE")),
            path_identity_key(Path::new("c:/apps/obs.exe"))
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn canonical_windows_paths_use_dos_and_unc_forms_without_verbatim_prefixes() {
        assert_eq!(
            super::normalize_canonical_path(PathBuf::from(r"\\?\C:\Apps\app.exe")),
            PathBuf::from(r"C:\Apps\app.exe")
        );
        assert_eq!(
            super::normalize_canonical_path(PathBuf::from(r"\\?\UNC\server\share\app.exe")),
            PathBuf::from(r"\\server\share\app.exe")
        );
    }

    #[test]
    fn next_order_follows_highest_existing_value() {
        assert_eq!(next_order(&[]), 0);
        assert_eq!(next_order(&[item(4), item(9), item(2)]), 10);
        assert_eq!(next_order(&[item(u32::MAX)]), u32::MAX);
    }

    #[tokio::test]
    async fn workers_build_multiple_items_in_selection_order() {
        let executable = TemporaryFile::application("EXE");
        let shortcut = TemporaryFile::application("lnk");

        let built = build_for_test(
            Vec::new(),
            vec![
                executable.0.to_string_lossy().into_owned(),
                shortcut.0.to_string_lossy().into_owned(),
            ],
            Arc::new(SystemIconExtractor),
            2,
            Duration::from_secs(1),
        )
        .await
        .expect("build launcher items");

        let items = built.items;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].order, 0);
        assert_eq!(items[1].order, 1);
        assert_eq!(items[0].kind, LauncherItemKind::Application);
    }

    #[tokio::test]
    async fn workers_skip_an_application_that_is_already_registered() {
        let executable = TemporaryFile::application("exe");
        let existing = build_for_test(
            Vec::new(),
            vec![executable.0.to_string_lossy().into_owned()],
            Arc::new(SystemIconExtractor),
            1,
            Duration::from_secs(1),
        )
        .await
        .expect("build initial launcher item")
        .items;
        let duplicate_items = build_for_test(
            existing,
            vec![executable.0.to_string_lossy().into_owned()],
            Arc::new(SystemIconExtractor),
            1,
            Duration::from_secs(1),
        )
        .await
        .expect("duplicates can be ignored while adding other selected apps")
        .items;

        assert!(duplicate_items.is_empty());
    }

    #[tokio::test]
    async fn worker_timeout_returns_before_a_stalled_extractor_finishes() {
        let executable = TemporaryFile::application("exe");
        let started_at = Instant::now();
        let error = build_for_test(
            Vec::new(),
            vec![executable.0.to_string_lossy().into_owned()],
            Arc::new(DelayedExtractor {
                result: Ok(None),
                delay: Duration::from_millis(250),
                started: None,
                active: None,
                peak_active: None,
            }),
            1,
            Duration::from_millis(30),
        )
        .await
        .expect_err("stalled extractor must time out");

        assert!(error.contains("タイムアウト"));
        assert!(started_at.elapsed() < Duration::from_millis(180));
        tokio::time::sleep(Duration::from_millis(260)).await;
    }

    #[tokio::test]
    async fn worker_failure_uses_a_generic_icon_and_records_the_reason() {
        let executable = TemporaryFile::application("exe");
        let built = build_for_test(
            Vec::new(),
            vec![executable.0.to_string_lossy().into_owned()],
            Arc::new(DelayedExtractor {
                result: Err("PowerShell のアイコン抽出に失敗しました: access denied".to_string()),
                delay: Duration::ZERO,
                started: None,
                active: None,
                peak_active: None,
            }),
            1,
            Duration::from_secs(1),
        )
        .await
        .expect("a failed icon extractor does not reject the application");

        assert_eq!(built.items[0].icon_data_url, None);
        assert!(built.icon_warnings[0].message.contains("access denied"));
    }

    #[tokio::test]
    async fn workers_never_exceed_the_configured_parallelism_limit() {
        let files = (0..8)
            .map(|_| TemporaryFile::application("exe"))
            .collect::<Vec<_>>();
        let active = Arc::new(AtomicUsize::new(0));
        let peak_active = Arc::new(AtomicUsize::new(0));
        let built = build_for_test(
            Vec::new(),
            files
                .iter()
                .map(|file| file.0.to_string_lossy().into_owned())
                .collect(),
            Arc::new(DelayedExtractor {
                result: Ok(None),
                delay: Duration::from_millis(40),
                started: None,
                active: Some(Arc::clone(&active)),
                peak_active: Some(Arc::clone(&peak_active)),
            }),
            2,
            Duration::from_secs(1),
        )
        .await
        .expect("bounded workers finish all items");

        assert_eq!(built.items.len(), files.len());
        assert_eq!(peak_active.load(Ordering::SeqCst), 2);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn worker_result_merges_concurrent_launcher_changes_and_discards_a_duplicate_target() {
        let first = TemporaryFile::application("exe");
        let concurrent = TemporaryFile::application("exe");
        let new = TemporaryFile::application("exe");
        let snapshot = build_for_test(
            Vec::new(),
            vec![first.0.to_string_lossy().into_owned()],
            Arc::new(SystemIconExtractor),
            1,
            Duration::from_secs(1),
        )
        .await
        .expect("build from the initial snapshot")
        .items;
        let latest = build_for_test(
            snapshot.clone(),
            vec![concurrent.0.to_string_lossy().into_owned()],
            Arc::new(SystemIconExtractor),
            1,
            Duration::from_secs(1),
        )
        .await
        .expect("build concurrent item")
        .items;
        let additions = build_for_test(
            snapshot.clone(),
            vec![
                concurrent.0.to_string_lossy().into_owned(),
                new.0.to_string_lossy().into_owned(),
            ],
            Arc::new(SystemIconExtractor),
            2,
            Duration::from_secs(1),
        )
        .await
        .expect("build requested items");
        let mut current = snapshot;
        current.extend(latest);

        let merged = merge_new_launcher_items(&current, additions.items).unwrap();

        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0].target,
            new.0
                .canonicalize()
                .map(super::normalize_canonical_path)
                .expect("canonical path")
                .to_string_lossy()
        );
        assert_eq!(merged[0].order, 2);
    }

    #[tokio::test]
    async fn worker_icon_extraction_runs_after_the_settings_snapshot_releases_its_lock() {
        let executable = TemporaryFile::application("exe");
        let settings = Arc::new(Mutex::new(AppSettings::default()));
        let snapshot = launcher_items_snapshot(&settings).expect("snapshot launcher settings");
        let started = Arc::new(AtomicBool::new(false));
        let target = executable.0.to_string_lossy().into_owned();

        let worker = tokio::spawn(build_for_test(
            snapshot,
            vec![target],
            Arc::new(DelayedExtractor {
                result: Ok(None),
                delay: Duration::from_millis(100),
                started: Some(Arc::clone(&started)),
                active: None,
                peak_active: None,
            }),
            1,
            Duration::from_secs(1),
        ));
        while !started.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
        assert!(
            settings.try_lock().is_ok(),
            "settings lock is free during extraction"
        );
        worker
            .await
            .expect("join extraction worker")
            .expect("build launcher item");
    }

    #[cfg(unix)]
    #[test]
    fn timed_out_child_is_killed_and_reaped() {
        let mut child = std::process::Command::new("sh")
            .args(["-c", "exec sleep 30"])
            .spawn()
            .expect("start stalled child");
        let started_at = Instant::now();

        let result = wait_for_child_exit(&mut child, Duration::from_millis(50));

        assert!(started_at.elapsed() < Duration::from_secs(1));
        match result {
            Err(ChildExitWaitError::TimedOut { termination: Ok(_) }) => {}
            Err(ChildExitWaitError::TimedOut {
                termination: Err(error),
            }) => panic!("timed out child was not terminated and reaped: {error}"),
            Err(ChildExitWaitError::Wait(error)) => {
                panic!("stalled child status could not be read: {error}")
            }
            Ok(status) => panic!("stalled child unexpectedly exited: {status}"),
        }
        assert!(child.try_wait().expect("read child status").is_some());
    }

    #[test]
    fn rejects_reserved_website_items_until_supported() {
        let website = LauncherItem {
            id: "website-1".to_string(),
            kind: LauncherItemKind::Website,
            target: "https://example.com".to_string(),
            display_name: "Example".to_string(),
            icon_data_url: None,
            background_color: None,
            group_id: None,
            order: 0,
        };

        let error = normalize_launcher_items(vec![website])
            .expect_err("website support is intentionally reserved");

        assert!(error.contains("まだ登録できません"));
    }
}

#[cfg(test)]
pub(crate) mod bounds_tests;
