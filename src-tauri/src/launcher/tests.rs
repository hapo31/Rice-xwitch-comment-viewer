#[cfg(all(feature = "app", not(target_os = "windows")))]
use super::launch_items;
use super::LauncherCapabilities;
use super::{
    build_new_items_in_workers_with_extractor, derive_display_name, is_supported_application_path,
    launcher_items_snapshot, merge_new_launcher_items, next_order,
    normalize_launcher_icon_data_url, normalize_launcher_items, path_identity_key,
    wait_for_child_exit, ChildExitWaitError, LauncherIconExtractor, LauncherItem, LauncherItemKind,
    LauncherWorkerConfig,
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

struct NoIconExtractor;
impl LauncherIconExtractor for NoIconExtractor {
    fn extract(&self, _: &Path) -> Result<Option<String>, super::IconExtractionError> {
        Ok(None)
    }
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
        normalize_launcher_icon_data_url(Some("data:image/svg+xml;base64,PHN2Zz4=".to_string())),
        None
    );
    assert_eq!(
        normalize_launcher_icon_data_url(Some("data:image/png;base64,iVBORw0KGgo!".to_string())),
        None
    );
    assert_eq!(
        normalize_launcher_icon_data_url(Some("data:image/png;base64,iVBORw0KGgo=".to_string())),
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
        Arc::new(NoIconExtractor),
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
        Arc::new(NoIconExtractor),
        1,
        Duration::from_secs(1),
    )
    .await
    .expect("build initial launcher item")
    .items;
    let duplicate_items = build_for_test(
        existing,
        vec![executable.0.to_string_lossy().into_owned()],
        Arc::new(NoIconExtractor),
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
        Arc::new(NoIconExtractor),
        1,
        Duration::from_secs(1),
    )
    .await
    .expect("build from the initial snapshot")
    .items;
    let latest = build_for_test(
        snapshot.clone(),
        vec![concurrent.0.to_string_lossy().into_owned()],
        Arc::new(NoIconExtractor),
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
        Arc::new(NoIconExtractor),
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
