#[cfg(feature = "app")]
use crate::app_events::{emit_app_log, AppLogLevel};
#[cfg(feature = "app")]
use crate::settings::{AppState, SettingsStore};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
#[cfg(all(feature = "app", target_os = "windows"))]
use std::process::{Command, Stdio};

const MAX_LAUNCHER_ITEMS: usize = 200;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSettings {
    #[serde(default)]
    pub items: Vec<LauncherItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LauncherItem {
    pub id: String,
    pub kind: LauncherItemKind,
    pub target: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
#[serde(rename_all = "camelCase")]
pub struct LauncherSettingsPatch {
    pub items: Option<Vec<LauncherItem>>,
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
        return Err(format!(
            "ランチャーに登録できるアプリは最大 {MAX_LAUNCHER_ITEMS} 件です。"
        ));
    }

    let mut ids = HashSet::with_capacity(items.len());
    let mut targets = HashSet::with_capacity(items.len());
    let mut normalized = Vec::with_capacity(items.len());

    for mut item in items {
        if item.kind != LauncherItemKind::Application {
            return Err("Webサイトのリンクはまだ登録できません。".to_string());
        }

        item.id = item.id.trim().to_string();
        if item.id.is_empty() {
            return Err("ランチャー項目の ID が空です。".to_string());
        }
        if !ids.insert(item.id.clone()) {
            return Err(format!("ランチャー項目の ID が重複しています: {}", item.id));
        }

        let target = validate_application_target(&item.target)?;
        let target_key = path_identity_key(&target);
        if !targets.insert(target_key) {
            return Err(format!(
                "同じアプリが複数登録されています: {}",
                target.display()
            ));
        }

        item.target = target.to_string_lossy().into_owned();
        item.display_name = normalize_display_name(&item.display_name, &target);
        item.icon_data_url = normalize_optional_text(item.icon_data_url);
        item.background_color = normalize_optional_text(item.background_color);
        item.group_id = normalize_optional_text(item.group_id);
        normalized.push(item);
    }

    Ok(normalized)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn normalize_display_name(value: &str, target: &Path) -> String {
    let value = value.trim();
    if value.is_empty() {
        derive_display_name(target)
    } else {
        value.chars().take(120).collect()
    }
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

fn build_new_items(
    existing: &[LauncherItem],
    raw_targets: Vec<String>,
) -> Result<Vec<LauncherItem>, String> {
    if raw_targets.is_empty() {
        return Err("追加するアプリを選択してください。".to_string());
    }
    let mut target_keys = existing
        .iter()
        .map(|item| path_identity_key(Path::new(&item.target)))
        .collect::<HashSet<_>>();
    let mut item_ids = existing
        .iter()
        .map(|item| item.id.clone())
        .collect::<HashSet<_>>();
    let mut order = next_order(existing);
    let mut new_items = Vec::with_capacity(raw_targets.len());

    for raw_target in raw_targets {
        let target = validate_application_target(&raw_target)?;
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
            icon_data_url: extract_icon_data_url(&target),
            background_color: None,
            group_id: None,
            order,
        });
        order = order.saturating_add(1);
    }

    Ok(new_items)
}

#[cfg(all(feature = "app", target_os = "windows"))]
fn extract_icon_data_url(target: &Path) -> Option<String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const MAX_ICON_BASE64_LENGTH: usize = 2_000_000;
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

    let child = Command::new("powershell.exe")
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
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }

    let encoded = String::from_utf8(output.stdout).ok()?;
    let encoded = encoded.trim();
    if encoded.is_empty() || encoded.len() > MAX_ICON_BASE64_LENGTH {
        return None;
    }
    Some(format!("data:image/png;base64,{encoded}"))
}

#[cfg(any(not(feature = "app"), not(target_os = "windows")))]
fn extract_icon_data_url(_target: &Path) -> Option<String> {
    None
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn launcher_add(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
    paths: Vec<String>,
) -> Result<Vec<LauncherItem>, String> {
    let mut settings = state.settings.lock().map_err(|error| error.to_string())?;
    let new_items = build_new_items(&settings.launcher.items, paths)?;
    let added_count = new_items.len();
    let previous_items = settings.launcher.items.clone();
    settings.launcher.items.extend(new_items);

    if let Err(error) = SettingsStore::save(&app, &settings) {
        settings.launcher.items = previous_items;
        return Err(format!("ランチャーの設定を保存できませんでした: {error}"));
    }

    let items = settings.launcher.items.clone();
    drop(settings);
    emit_app_log(
        &app,
        AppLogLevel::Info,
        format!("ランチャーにアプリを {added_count} 件追加しました。"),
    );
    Ok(items)
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

    let removed = settings.launcher.items.remove(index);
    if let Err(error) = SettingsStore::save(&app, &settings) {
        settings.launcher.items.insert(index, removed);
        return Err(format!("ランチャーの設定を保存できませんでした: {error}"));
    }

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
    use super::{
        build_new_items, derive_display_name, is_supported_application_path, next_order,
        normalize_launcher_items, path_identity_key, LauncherItem, LauncherItemKind,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TemporaryFile(PathBuf);

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

    #[test]
    fn recognizes_supported_extensions_without_case_sensitivity() {
        assert!(is_supported_application_path(Path::new("app.exe")));
        assert!(is_supported_application_path(Path::new("APP.EXE")));
        assert!(is_supported_application_path(Path::new("shortcut.LnK")));
        assert!(!is_supported_application_path(Path::new("script.bat")));
        assert!(!is_supported_application_path(Path::new("app.exe.txt")));
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

    #[test]
    fn next_order_follows_highest_existing_value() {
        assert_eq!(next_order(&[]), 0);
        assert_eq!(next_order(&[item(4), item(9), item(2)]), 10);
        assert_eq!(next_order(&[item(u32::MAX)]), u32::MAX);
    }

    #[test]
    fn builds_multiple_items_in_selection_order() {
        let executable = TemporaryFile::application("EXE");
        let shortcut = TemporaryFile::application("lnk");

        let items = build_new_items(
            &[],
            vec![
                executable.0.to_string_lossy().into_owned(),
                shortcut.0.to_string_lossy().into_owned(),
            ],
        )
        .expect("build launcher items");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].order, 0);
        assert_eq!(items[1].order, 1);
        assert_eq!(items[0].kind, LauncherItemKind::Application);
    }

    #[test]
    fn skips_an_application_that_is_already_registered() {
        let executable = TemporaryFile::application("exe");
        let existing = build_new_items(&[], vec![executable.0.to_string_lossy().into_owned()])
            .expect("build initial launcher item");

        let duplicate_items =
            build_new_items(&existing, vec![executable.0.to_string_lossy().into_owned()])
                .expect("duplicates can be ignored while adding other selected apps");

        assert!(duplicate_items.is_empty());
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
