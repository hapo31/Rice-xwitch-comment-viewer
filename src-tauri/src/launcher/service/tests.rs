//! All-OS tests of the same service used by IPC: no real file, COM or process.
use super::*;
use crate::launcher::ports::{IconExtractionError, IconExtractionWarning};
use crate::launcher::repository::AppSettingsRepository;
use crate::settings::AppSettings;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tokio::sync::{oneshot, Semaphore};

#[derive(Default)]
struct FakeRepository {
    settings: Mutex<AppSettings>,
    saved: Mutex<Vec<AppSettings>>,
    fail_save: AtomicBool,
    fail_read: AtomicBool,
}
impl SettingsRepository for FakeRepository {
    fn snapshot(&self) -> Result<Vec<LauncherItem>, String> {
        if self.fail_read.load(Ordering::SeqCst) {
            return Err("読み込み失敗fixture".into());
        }
        Ok(self.settings.lock().unwrap().launcher.items.clone())
    }
    fn update(
        &self,
        mutation: &mut dyn FnMut(&mut Vec<LauncherItem>) -> Result<(), String>,
    ) -> Result<Vec<LauncherItem>, String> {
        AppSettingsRepository {
            settings: &self.settings,
            persist: |candidate: &AppSettings| {
                if self.fail_save.load(Ordering::SeqCst) {
                    return Err("保存失敗fixture".into());
                }
                self.saved.lock().unwrap().push(candidate.clone());
                Ok(())
            },
        }
        .update(mutation)
    }
}

#[derive(Default)]
struct FakeResolver {
    calls: Mutex<Vec<String>>,
}
impl ApplicationTargetResolver for FakeResolver {
    fn resolve(&self, target: &str) -> Result<PathBuf, String> {
        self.calls.lock().unwrap().push(target.into());
        if target.ends_with("broken.lnk") {
            return Err("ショートカットの参照先が見つかりません。fixture".into());
        }
        Ok(PathBuf::from(target.trim()))
    }
}

#[derive(Default)]
struct FakeLauncher {
    calls: Mutex<Vec<PathBuf>>,
}
impl ApplicationLauncher for FakeLauncher {
    fn launch(&self, target: &Path, context: &LaunchContext) -> Result<(), String> {
        context.ensure_active()?;
        self.calls.lock().unwrap().push(target.into());
        if target.ends_with("denied.exe") {
            return Err("spawn failure fixture".into());
        }
        Ok(())
    }
}

struct FakeExtractor(Result<Option<String>, IconExtractionError>);
impl IconExtractor for FakeExtractor {
    fn extract(&self, _: &Path) -> Result<Option<String>, IconExtractionError> {
        self.0.clone()
    }
}

#[derive(Default)]
struct RecordingEvents {
    added: Mutex<Vec<usize>>,
    removed: Mutex<Vec<String>>,
    launched: Mutex<Vec<LauncherLaunchResult>>,
    warnings: Mutex<Vec<IconExtractionWarning>>,
}
impl LauncherEventSink for RecordingEvents {
    fn added(&self, count: usize) {
        self.added.lock().unwrap().push(count);
    }
    fn removed(&self, item: &LauncherItem) {
        self.removed.lock().unwrap().push(item.id.clone());
    }
    fn launched(&self, result: &LauncherLaunchResult) {
        self.launched.lock().unwrap().push(result.clone());
    }
    fn icon_warnings(&self, warnings: &[IconExtractionWarning]) {
        self.warnings.lock().unwrap().extend_from_slice(warnings);
    }
}

fn workers(count: usize) -> LauncherWorkerConfig {
    LauncherWorkerConfig {
        worker_limit: count,
        worker_pool: Arc::new(Semaphore::new(count)),
        acquire_timeout: Duration::from_secs(6),
        job_timeout: Duration::from_secs(7),
    }
}

fn runtime(
    supported: bool,
    extractor: Arc<dyn IconExtractor>,
    config: LauncherWorkerConfig,
) -> (LauncherRuntime, Arc<FakeResolver>, Arc<FakeLauncher>) {
    let resolver = Arc::new(FakeResolver::default());
    let launcher = Arc::new(FakeLauncher::default());
    (
        LauncherRuntime::new(
            LauncherCapabilities::for_platform(supported),
            resolver.clone(),
            extractor,
            launcher.clone(),
            config,
        ),
        resolver,
        launcher,
    )
}

fn item(id: &str, target: &str, order: u32) -> LauncherItem {
    LauncherItem {
        id: id.into(),
        kind: LauncherItemKind::Application,
        target: target.into(),
        display_name: id.into(),
        order,
        icon_data_url: None,
        background_color: None,
        group_id: None,
    }
}

#[tokio::test]
async fn extractor_timeout_and_failure_fall_back_and_log_only_after_commit() {
    for message in ["アイコン抽出timeout fixture", "アイコン抽出failure fixture"] {
        let (runtime, resolver, _) = runtime(
            true,
            Arc::new(FakeExtractor(Err(message.to_string().into()))),
            workers(1),
        );
        let repository = FakeRepository::default();
        let events = RecordingEvents::default();
        let items = runtime
            .service(&repository, &events)
            .add(vec!["/fake/a.exe".into()])
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].icon_data_url.is_none());
        assert_eq!(repository.saved.lock().unwrap().len(), 1);
        assert_eq!(*events.added.lock().unwrap(), [1]);
        let warnings = events.warnings.lock().unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].message, message);
        assert_eq!(warnings[0].target, PathBuf::from("/fake/a.exe"));
        assert_eq!(*resolver.calls.lock().unwrap(), ["/fake/a.exe"]);
    }
}

#[tokio::test]
async fn invalid_icon_or_resource_limit_rejects_whole_add_without_save_or_success_log() {
    for result in [
        Err(IconExtractionError::ResourceLimit),
        Ok(Some("https://example.com/tracker.png".into())),
    ] {
        let (runtime, _, _) = runtime(true, Arc::new(FakeExtractor(result)), workers(1));
        let repository = FakeRepository::default();
        let events = RecordingEvents::default();
        let error = runtime
            .service(&repository, &events)
            .add(vec!["/fake/a.exe".into()])
            .await
            .unwrap_err();
        assert!(error.contains("保存していません"));
        assert!(repository.snapshot().unwrap().is_empty());
        assert!(repository.saved.lock().unwrap().is_empty());
        assert!(events.added.lock().unwrap().is_empty());
        assert!(events.warnings.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn save_failure_rolls_back_add_remove_and_other_sections_without_success_events() {
    let (runtime, _, _) = runtime(
        true,
        Arc::new(FakeExtractor(Err("抽出failure".to_string().into()))),
        workers(1),
    );
    let repository = FakeRepository::default();
    {
        let mut settings = repository.settings.lock().unwrap();
        settings.twitch.channel_login = "keep-channel".into();
        settings.speech.bouyomi_port = 51001;
        settings.launcher.items = vec![item("old", "/fake/old.exe", 4)];
    }
    let before = serde_json::to_value(&*repository.settings.lock().unwrap()).unwrap();
    repository.fail_save.store(true, Ordering::SeqCst);
    let events = RecordingEvents::default();
    let service = runtime.service(&repository, &events);
    assert!(service
        .add(vec!["/fake/a.exe".into()])
        .await
        .unwrap_err()
        .contains("保存失敗"));
    assert!(service.remove("old").unwrap_err().contains("保存失敗"));
    assert_eq!(
        serde_json::to_value(&*repository.settings.lock().unwrap()).unwrap(),
        before
    );
    assert!(repository.saved.lock().unwrap().is_empty());
    assert!(events.added.lock().unwrap().is_empty());
    assert!(events.removed.lock().unwrap().is_empty());
    assert!(events.warnings.lock().unwrap().is_empty());
}

#[tokio::test]
async fn broken_shortcut_rejects_registration_before_save_and_maps_launch_failure() {
    let (runtime, resolver, launcher) =
        runtime(true, Arc::new(FakeExtractor(Ok(None))), workers(1));
    let repository = FakeRepository::default();
    let events = RecordingEvents::default();
    let error = runtime
        .service(&repository, &events)
        .add(vec!["/fake/broken.lnk".into()])
        .await
        .unwrap_err();
    assert!(error.contains("参照先"));
    assert!(repository.saved.lock().unwrap().is_empty());
    assert!(events.added.lock().unwrap().is_empty());
    repository.settings.lock().unwrap().launcher.items =
        vec![item("broken", "/fake/broken.lnk", 0)];
    let result = runtime.service(&repository, &events).launch("broken").await;
    assert_eq!(result.launched_count, 0);
    assert_eq!(result.failures[0].item_id, "broken");
    assert!(result.failures[0].message.contains("参照先"));
    assert_eq!(resolver.calls.lock().unwrap().len(), 2);
    assert!(launcher.calls.lock().unwrap().is_empty());
    assert_eq!(*events.launched.lock().unwrap(), [result]);
}

#[tokio::test]
async fn launch_all_keeps_selection_order_and_partial_failures_without_website_adapter_dispatch() {
    let (runtime, resolver, launcher) =
        runtime(true, Arc::new(FakeExtractor(Ok(None))), workers(1));
    let repository = FakeRepository::default();
    let mut website = item("website", "https://example.com", 3);
    website.kind = LauncherItemKind::Website;
    repository.settings.lock().unwrap().launcher.items = vec![
        item("denied", "/fake/denied.exe", 2),
        website,
        item("ok", "/fake/ok.exe", 0),
        item("broken", "/fake/broken.lnk", 1),
    ];
    let events = RecordingEvents::default();
    let result = runtime.service(&repository, &events).launch_all().await;
    assert_eq!(result.launched_count, 1);
    assert_eq!(
        result
            .failures
            .iter()
            .map(|item| item.item_id.as_str())
            .collect::<Vec<_>>(),
        ["broken", "denied", "website"]
    );
    assert!(result.failures[1].message.contains("spawn failure fixture"));
    assert_eq!(
        *resolver.calls.lock().unwrap(),
        ["/fake/ok.exe", "/fake/broken.lnk", "/fake/denied.exe"]
    );
    assert_eq!(
        *launcher.calls.lock().unwrap(),
        [
            PathBuf::from("/fake/ok.exe"),
            PathBuf::from("/fake/denied.exe")
        ]
    );
    assert!(repository.saved.lock().unwrap().is_empty());
    assert_eq!(*events.launched.lock().unwrap(), [result]);
}

#[tokio::test]
async fn unsupported_platform_calls_no_os_adapters_but_still_removes_saved_items() {
    let (runtime, resolver, launcher) =
        runtime(false, Arc::new(FakeExtractor(Ok(None))), workers(1));
    let repository = FakeRepository::default();
    repository.settings.lock().unwrap().launcher.items = vec![item("old", "/fake/old.exe", 0)];
    let events = RecordingEvents::default();
    let service = runtime.service(&repository, &events);
    assert!(service
        .add(vec!["/fake/a.exe".into()])
        .await
        .unwrap_err()
        .contains("Windows版"));
    assert!(service.launch("old").await.failures[0]
        .message
        .contains("Windows版"));
    assert!(resolver.calls.lock().unwrap().is_empty());
    assert!(launcher.calls.lock().unwrap().is_empty());
    assert!(repository.saved.lock().unwrap().is_empty());
    assert!(service.remove(" old ").unwrap().is_empty());
    assert_eq!(*events.removed.lock().unwrap(), ["old"]);
}

#[tokio::test]
async fn missing_item_and_repository_read_failure_use_the_same_service_result_contract() {
    let (runtime, _, launcher) = runtime(true, Arc::new(FakeExtractor(Ok(None))), workers(1));
    let repository = FakeRepository::default();
    let events = RecordingEvents::default();
    let service = runtime.service(&repository, &events);
    let missing = service.launch("unknown").await;
    assert_eq!(missing.failures[0].item_id, "unknown");
    assert!(missing.failures[0].message.contains("見つかりません"));
    assert!(service.remove("unknown").is_err());
    repository.fail_read.store(true, Ordering::SeqCst);
    for result in [service.launch("known").await, service.launch_all().await] {
        assert_eq!(result.launched_count, 0);
        assert!(result.failures[0].message.contains("読み込み失敗fixture"));
    }
    assert!(launcher.calls.lock().unwrap().is_empty());
    assert!(repository.saved.lock().unwrap().is_empty());
    assert!(events.removed.lock().unwrap().is_empty());
}

struct GatedExtractor {
    started: Mutex<Option<oneshot::Sender<()>>>,
    release: Mutex<mpsc::Receiver<()>>,
    exited: Mutex<Option<oneshot::Sender<()>>>,
}

struct GatedResolver {
    gate: Arc<GatedExtractor>,
}
impl ApplicationTargetResolver for GatedResolver {
    fn resolve(&self, raw: &str) -> Result<PathBuf, String> {
        self.gate
            .extract(Path::new(raw))
            .map_err(|error| format!("{error:?}"))?;
        Ok(raw.into())
    }
}

#[tokio::test]
async fn launch_timeout_retains_permit_and_cancels_late_resolver_before_process_start() {
    let (gate, started, exited, mut release) = gated();
    let config = workers(1);
    let pool = config.worker_pool.clone();
    let launcher = Arc::new(FakeLauncher::default());
    let runtime = LauncherRuntime::new(
        LauncherCapabilities::for_platform(true),
        Arc::new(GatedResolver { gate }),
        Arc::new(FakeExtractor(Ok(None))),
        launcher.clone(),
        config,
    );
    let repository = FakeRepository::default();
    repository.settings.lock().unwrap().launcher.items = vec![item("slow", "/fake/slow.lnk", 0)];
    let events = RecordingEvents::default();
    let service = runtime.service(&repository, &events);
    let mut launch = Box::pin(service.launch("slow"));
    tokio::select! { result = &mut launch => panic!("premature result: {result:?}"), ready = started => ready.unwrap() }
    tokio::time::pause();
    tokio::time::advance(Duration::from_millis(7001)).await;
    let result = launch.await;
    assert_eq!(result.launched_count, 0);
    assert!(result.failures[0]
        .message
        .contains("既に起動している可能性"));
    assert_eq!(pool.available_permits(), 0);
    let mut second = Box::pin(service.launch("slow"));
    assert!(futures_util::poll!(&mut second).is_pending());
    tokio::time::advance(Duration::from_millis(6001)).await;
    let busy = second.await;
    assert_eq!(busy.launched_count, 0);
    assert!(busy.failures[0]
        .message
        .contains("起動要求は送っていません"));
    release.release();
    exited.await.unwrap();
    // Acquire the permit rather than merely waiting for the resolver's signal:
    // it is released only after the cancelled worker itself has returned.
    let _permit = pool.acquire().await.unwrap();
    assert!(launcher.calls.lock().unwrap().is_empty());
    assert!(repository.saved.lock().unwrap().is_empty());
    assert_eq!(*events.launched.lock().unwrap(), [result, busy]);
}
impl IconExtractor for GatedExtractor {
    fn extract(&self, _: &Path) -> Result<Option<String>, IconExtractionError> {
        if let Some(started) = self.started.lock().unwrap().take() {
            let _ = started.send(());
        }
        self.release.lock().unwrap().recv().unwrap();
        if let Some(exited) = self.exited.lock().unwrap().take() {
            let _ = exited.send(());
        }
        Ok(None)
    }
}
struct ReleaseOnDrop(Option<mpsc::Sender<()>>);
impl ReleaseOnDrop {
    fn release(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.release();
    }
}
fn gated() -> (
    Arc<GatedExtractor>,
    oneshot::Receiver<()>,
    oneshot::Receiver<()>,
    ReleaseOnDrop,
) {
    let (started, ready) = oneshot::channel();
    let (exited, done) = oneshot::channel();
    let (sender, release) = mpsc::channel();
    (
        Arc::new(GatedExtractor {
            started: Mutex::new(Some(started)),
            release: Mutex::new(release),
            exited: Mutex::new(Some(exited)),
        }),
        ready,
        done,
        ReleaseOnDrop(Some(sender)),
    )
}

#[tokio::test]
async fn virtual_timeout_retains_global_permit_and_never_publishes_late_blocking_result() {
    let (extractor, started, exited, mut release) = gated();
    let config = workers(1);
    let pool = config.worker_pool.clone();
    let (runtime, resolver, _) = runtime(true, extractor, config);
    let runtime = Arc::new(runtime);
    let repository = Arc::new(FakeRepository::default());
    let events = Arc::new(RecordingEvents::default());
    let task = {
        let runtime = runtime.clone();
        let repository = repository.clone();
        let events = events.clone();
        tokio::spawn(async move {
            runtime
                .service(repository.as_ref(), events.as_ref())
                .add(vec!["/fake/a.exe".into()])
                .await
        })
    };
    // Wait for the blocking adapter handshake before pausing: no wall-clock
    // sleep, spin/yield loop, scheduler race or real filesystem timeout.
    started.await.unwrap();
    assert!(repository.settings.try_lock().is_ok());
    tokio::time::pause();
    // Tokio timers have millisecond resolution; cross the rounded deadline.
    tokio::time::advance(Duration::from_millis(7001)).await;
    assert!(task.await.unwrap().unwrap_err().contains("タイムアウト"));
    assert_eq!(pool.available_permits(), 0);
    let service = runtime.service(repository.as_ref(), events.as_ref());
    let waiting_add = service.add(vec!["/fake/b.exe".into()]);
    tokio::pin!(waiting_add);
    // Blocking tasks inhibit Tokio's automatic clock advance. Poll once to
    // register the acquire deadline, then advance it explicitly as well.
    assert!(futures_util::poll!(&mut waiting_add).is_pending());
    tokio::time::advance(Duration::from_millis(6001)).await;
    let error = waiting_add.await.unwrap_err();
    assert!(error.contains("混み合っています"));
    assert_eq!(resolver.calls.lock().unwrap().len(), 1);
    release.release();
    exited.await.unwrap();
    let permit = pool.clone().acquire_owned().await.unwrap();
    drop(permit);
    assert_eq!(pool.available_permits(), 1);
    assert!(repository.snapshot().unwrap().is_empty());
    assert!(repository.saved.lock().unwrap().is_empty());
    assert!(events.added.lock().unwrap().is_empty());
}

#[tokio::test]
async fn extraction_allows_concurrent_service_add_remove_and_keeps_other_settings() {
    let (extractor, started, exited, mut release) = gated();
    let config = workers(2);
    let (first, _, _) = runtime(true, extractor, config.clone());
    let first = Arc::new(first);
    let (second, _, _) = runtime(true, Arc::new(FakeExtractor(Ok(None))), config);
    let repository = Arc::new(FakeRepository::default());
    repository.settings.lock().unwrap().launcher.items = vec![item("old", "/fake/old.exe", 0)];
    let events = Arc::new(RecordingEvents::default());
    let task = {
        let first = first.clone();
        let repository = repository.clone();
        let events = events.clone();
        tokio::spawn(async move {
            first
                .service(repository.as_ref(), events.as_ref())
                .add(vec!["/fake/a.exe".into()])
                .await
        })
    };
    started.await.unwrap();
    {
        let mut settings = repository
            .settings
            .try_lock()
            .expect("no settings lock during extraction");
        settings.twitch.channel_login = "concurrent-channel".into();
        settings.speech.bouyomi_port = 52001;
    }
    second
        .service(repository.as_ref(), events.as_ref())
        .remove("old")
        .unwrap();
    second
        .service(repository.as_ref(), events.as_ref())
        .add(vec!["/fake/a.exe".into(), "/fake/b.exe".into()])
        .await
        .unwrap();
    release.release();
    exited.await.unwrap();
    let items = task.await.unwrap().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items
            .iter()
            .map(|item| item.target.as_str())
            .collect::<Vec<_>>(),
        ["/fake/a.exe", "/fake/b.exe"]
    );
    assert_eq!(
        items.iter().map(|item| item.order).collect::<Vec<_>>(),
        [0, 1]
    );
    assert_ne!(items[0].id, items[1].id);
    assert_eq!(*events.added.lock().unwrap(), [2, 0]);
    assert_eq!(*events.removed.lock().unwrap(), ["old"]);
    let saved = repository.saved.lock().unwrap();
    assert_eq!(saved.len(), 3);
    for settings in saved.iter() {
        assert_eq!(settings.twitch.channel_login, "concurrent-channel");
        assert_eq!(settings.speech.bouyomi_port, 52001);
    }
}
