//! Real Windows COM links and CreateProcess, through the production service.
//! CI runs this normally (not ignored); all files/processes are fixture-owned.
use super::*;
use crate::launcher::model::{LauncherItem, LauncherItemKind, LauncherLaunchResult};
use crate::launcher::platform::process::capture_bounded;
use crate::launcher::ports::{IconExtractionWarning, LauncherEventSink};
use crate::launcher::repository::AppSettingsRepository;
use crate::launcher::LauncherRuntime;
use crate::settings::AppSettings;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rice-shortcut-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn cleanup(&self) -> Result<(), std::io::Error> {
        std::env::remove_var("RICE_LAUNCH_PROBE_OUT");
        for _ in 0..40 {
            match fs::remove_dir_all(&self.0) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(_) => std::thread::sleep(Duration::from_millis(25)),
            }
        }
        fs::remove_dir_all(&self.0)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Never target a user directory. A probe may still be exiting after its
        // report, so allow a short bounded retry of this exact private directory.
        if let Err(error) = self.cleanup() {
            eprintln!(
                "fixture cleanup could not finish: {}: {error}",
                self.0.display()
            );
        }
    }
}
struct NoEvents;
impl LauncherEventSink for NoEvents {
    fn added(&self, _: usize) {}
    fn removed(&self, _: &LauncherItem) {}
    fn launched(&self, _: &LauncherLaunchResult) {}
    fn icon_warnings(&self, _: &[IconExtractionWarning]) {}
}

fn make_link(path: &Path, target: &Path, arguments: &str, cwd: &Path) {
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"
$ErrorActionPreference = 'Stop'
[Console]::Error.WriteLine('fixture-stage: PowerShell started')
$link = (New-Object -ComObject WScript.Shell).CreateShortcut($env:RICE_TEST_LINK)
[Console]::Error.WriteLine('fixture-stage: COM link created')
$link.TargetPath = $env:RICE_TEST_TARGET
$link.Arguments = $env:RICE_TEST_ARGS
$link.WorkingDirectory = $env:RICE_TEST_CWD
$link.IconLocation = $env:RICE_TEST_TARGET + ',0'
[Console]::Error.WriteLine('fixture-stage: properties assigned')
$link.Save()
[Console]::Error.WriteLine('fixture-stage: link saved')
"#,
        ])
        .creation_flags(0x0800_0000)
        .env("RICE_TEST_LINK", path)
        .env("RICE_TEST_TARGET", target)
        .env("RICE_TEST_ARGS", arguments)
        .env("RICE_TEST_CWD", cwd)
        .env_remove("PSModulePath");
    capture_bounded(command, Duration::from_secs(5), 4096).unwrap();
}

fn item(id: &str, path: &Path, order: u32) -> LauncherItem {
    LauncherItem {
        id: id.into(),
        kind: LauncherItemKind::Application,
        target: path.to_string_lossy().into_owned(),
        display_name: id.into(),
        icon_data_url: None,
        background_color: None,
        group_id: None,
        order,
    }
}
async fn read_probe(path: &Path) -> String {
    let started = Instant::now();
    loop {
        if let Ok(record) = fs::read_to_string(path) {
            if record.ends_with('\n') {
                return record;
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "target process did not report: {}",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn real_links_validate_target_arguments_workdir_permissions_and_partial_success() {
    let fixture = Fixture::new();
    let app_dir = fixture.0.join("配信 アプリ");
    let work_dir = fixture.0.join("別の 作業場所");
    fs::create_dir(&app_dir).unwrap();
    fs::create_dir(&work_dir).unwrap();
    let executable = app_dir.join("launcher probe.exe");
    let mut compile = Command::new("rustc");
    compile
        .arg("--edition=2021")
        .arg("--crate-name=rice_launcher_probe")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/launcher-probe.rs"))
        .arg("-o")
        .arg(&executable);
    capture_bounded(compile, Duration::from_secs(30), 4096).unwrap();
    let valid = fixture.0.join("valid link.lnk");
    let args = "\"日本語 空白\" & | < > ^ % $";
    make_link(&valid, &executable, args, &work_dir);
    let resolved = shortcut::resolve(&valid).unwrap();
    assert_eq!(resolved.executable, executable);
    assert_eq!(resolved.working_directory, work_dir);
    assert_eq!(resolved.metadata.arguments, args);
    assert_eq!(resolved.metadata.icon_source, executable.to_string_lossy());
    let settings = Mutex::new(AppSettings::default());
    let repository = AppSettingsRepository {
        settings: &settings,
        persist: |_: &AppSettings| -> Result<(), String> { panic!("launch must not save") },
    };
    let runtime = LauncherRuntime::default();
    let service = runtime.service(&repository, &NoEvents);
    settings.lock().unwrap().launcher.items = vec![item("valid", &valid, 0)];
    let output = fixture.0.join("valid record.txt");
    std::env::set_var("RICE_LAUNCH_PROBE_OUT", &output);
    assert_eq!(service.launch("valid").await.launched_count, 1);
    let expected = format!(
        "cwd={}\narg=日本語 空白\narg=&\narg=|\narg=<\narg=>\narg=^\narg=%\narg=$\n",
        work_dir.display()
    );
    assert_eq!(read_probe(&output).await, expected);

    // Re-read on every launch: changing cwd/arguments after registration takes
    // effect. An empty cwd explicitly falls back to the executable directory.
    make_link(&valid, &executable, "changed", Path::new(""));
    let output = fixture.0.join("changed record.txt");
    std::env::set_var("RICE_LAUNCH_PROBE_OUT", &output);
    assert_eq!(service.launch("valid").await.launched_count, 1);
    assert_eq!(
        read_probe(&output).await,
        format!("cwd={}\narg=changed\n", app_dir.display())
    );

    let broken = fixture.0.join("broken.lnk");
    make_link(&broken, &fixture.0.join("missing.exe"), "", &work_dir);
    let moved = fixture.0.join("moved.lnk");
    let old_target = app_dir.join("old.exe");
    fs::copy(&executable, &old_target).unwrap();
    make_link(&moved, &old_target, "", &work_dir);
    fs::rename(&old_target, app_dir.join("new.exe")).unwrap();
    let bad_cwd = fixture.0.join("bad cwd.lnk");
    make_link(
        &bad_cwd,
        &executable,
        "",
        &fixture.0.join("missing directory"),
    );
    let elevated = fixture.0.join("permission.lnk");
    make_link(&elevated, &executable, "", &work_dir);
    let mut bytes = fs::read(&elevated).unwrap();
    let flags = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) | 0x2000;
    bytes[20..24].copy_from_slice(&flags.to_le_bytes());
    fs::write(&elevated, bytes).unwrap();
    for (id, link, cause) in [
        ("broken", &broken, "リンク先"),
        ("moved", &moved, "リンク先"),
        ("cwd", &bad_cwd, "作業フォルダー"),
        ("permission", &elevated, "昇格要求は送っていません"),
    ] {
        let output = fixture.0.join(format!("failed-{id}.txt"));
        std::env::set_var("RICE_LAUNCH_PROBE_OUT", &output);
        settings.lock().unwrap().launcher.items = vec![item(id, link, 0)];
        let result = service.launch(id).await;
        assert_eq!(result.launched_count, 0, "{id}");
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].item_id, id);
        assert!(
            result.failures[0].message.contains(cause),
            "{}",
            result.failures[0].message
        );
        assert!(!output.exists(), "failed link must not launch the target");
    }
    let output = fixture.0.join("bulk record.txt");
    std::env::set_var("RICE_LAUNCH_PROBE_OUT", &output);
    settings.lock().unwrap().launcher.items = vec![
        item("broken", &broken, 1),
        item("direct", &executable, 0),
        item("moved", &moved, 2),
        item("permission", &elevated, 3),
    ];
    let result = service.launch_all().await;
    assert_eq!(result.launched_count, 1);
    assert_eq!(
        result
            .failures
            .iter()
            .map(|failure| failure.item_id.as_str())
            .collect::<Vec<_>>(),
        ["broken", "moved", "permission"]
    );
    assert_eq!(
        read_probe(&output).await,
        format!("cwd={}\n", app_dir.display())
    );
    println!("real Windows links: valid/refresh/broken/moved/arguments/cwd/permission/direct/partial all passed; process creation != app readiness");
    fixture
        .cleanup()
        .expect("isolated launch fixture must be removed");
}
