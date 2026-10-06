//! Explicit Windows CI fixture; runs the production builder and plugin, not a
//! second implementation. No debug command, account or release is created.
use crate::settings::{AppState, SettingsStore};
use crate::twitch::{AuthCredentialStore, AuthLoadResult, TwitchAuthState, TwitchAuthStore};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::Manager;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, IsIconic, PostMessageW, ShowWindow, SW_MINIMIZE, WM_CLOSE,
};

const FIXTURE: &str = "single_instance::windows_tests::native_instance_fixture";
const FUTURE_SETTINGS: &str = "{\"schemaVersion\":999,\"twitch\":{\"channelLogin\":\"future_channel\",\"autoConnect\":true},\"futureData\":{\"must\":\"preserve exactly\"}}\n";
const FUTURE_BACKUP: &str =
    "{\"schemaVersion\":1,\"twitch\":{\"channelLogin\":\"preserved_backup\"}}\n";

struct NoCredentials;
impl AuthCredentialStore for NoCredentials {
    fn load(&self) -> AuthLoadResult {
        AuthLoadResult {
            auth: None,
            notice: None,
        }
    }
    fn save(&self, _: &TwitchAuthState) -> anyhow::Result<Option<String>> {
        panic!("fixture must not save credentials")
    }
    fn clear(&self) -> anyhow::Result<()> {
        panic!("fixture must not clear credentials")
    }
}

#[test]
fn native_instance_fixture() {
    let Ok(id) = std::env::var("RICE_NATIVE_TEST_ID") else {
        return;
    };
    let mut context = tauri::generate_context!();
    context.config_mut().identifier = id;
    context.config_mut().build.dev_url = None;
    // Windows known-folder APIs need not honor APPDATA environment variables.
    // Use Tauri's native override for every app directory and webview storage.
    let root =
        std::path::PathBuf::from(std::env::var_os("RICE_NATIVE_TEST_ROOT").expect("isolated root"));
    context.config_mut().app.app_directories_override = Some(
        if std::env::var_os("RICE_SETTINGS_SCHEMA_NATIVE").is_some() {
            // Root overrides deliberately share data/local-data. Match normal
            // Windows separation so WebView cache is not mistaken for a
            // settings temporary file, while every path stays isolated.
            tauri::utils::config::AppDirectoriesOverride::Directories(
                tauri::utils::config::AppDirectoryOverrides {
                    config: Some(root.join("config")),
                    data: Some(root.join("settings")),
                    local_data: Some(root.join("webview")),
                    cache: Some(root.join("cache")),
                    log: Some(root.join("logs")),
                },
            )
        } else {
            tauri::utils::config::AppDirectoriesOverride::Root(root)
        },
    );
    let state = AppState {
        twitch_auth_store: TwitchAuthStore::with_backend(std::sync::Arc::new(NoCredentials)),
        ..AppState::default()
    };
    let builder = crate::app_builder_with_state(state);
    let builder = if std::env::var_os("RICE_SETTINGS_SCHEMA_NATIVE").is_some() {
        // Plugin setup precedes the unchanged production application setup.
        // Seed only this child's overridden app-data directory, before its
        // real SettingsStore::load obtains ownership and reads the file.
        builder.plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("settings-schema-fixture")
                .setup(|app, _| {
                    let directory = app.path().app_data_dir()?;
                    std::fs::create_dir_all(&directory)?;
                    std::fs::write(directory.join("settings.json"), FUTURE_SETTINGS)?;
                    std::fs::write(directory.join("settings.json.bak"), FUTURE_BACKUP)?;
                    Ok(())
                })
                .build(),
        )
    } else {
        builder
    };
    let app = builder
        .on_page_load(|webview, payload| {
            if std::env::var_os("RICE_SETTINGS_SCHEMA_NATIVE").is_some()
                && matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
            {
                let target = serde_json::to_string(
                    &std::env::current_exe()
                        .expect("native binary")
                        .to_string_lossy(),
                )
                .expect("target JSON");
                webview
                    .eval(&format!(
                        "window.__RICE_SCHEMA_TARGET = {target}; {}",
                        include_str!("../../tests/settings-schema-native.js")
                    ))
                    .expect("native schema script");
            }
            if std::env::var_os("RICE_LAUNCHER_NATIVE_BUDGET").is_some()
                && matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
            {
                webview
                    .eval(include_str!("../../tests/launcher-native-budget.js"))
                    .expect("native measurement script");
            }
        })
        .any_thread()
        .build(context)
        .expect("native production app setup");
    // Tauri creates configured windows and runs setup on the event loop's Ready
    // event, not in build(). A contender must exit in the plugin before Ready.
    app.run(|app, event| {
        if !matches!(event, tauri::RunEvent::Ready) { return; }
        assert_eq!(std::env::var("RICE_NATIVE_TEST_ROLE").expect("role"), "owner");
        let window = app.get_webview_window("main").expect("production main window");
        let hwnd = window.hwnd().expect("native HWND").0 as usize;
        if std::env::var_os("RICE_SETTINGS_SCHEMA_NATIVE").is_some() {
            let settings = app.state::<AppState>().settings.lock().expect("future settings").clone();
            assert!(!settings.twitch.auto_connect);
            assert!(settings.twitch.channel_login.is_empty());
            std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(60);
                let mut observed = String::new();
                loop {
                    let url = window.url().expect("native document URL");
                    if url.as_str() != observed {
                        println!("RICE_SCHEMA_STAGE {url}");
                        std::io::stdout().flush().expect("flush schema stage");
                        observed = url.to_string();
                    }
                    if let Some(fragment) = url.fragment().filter(|fragment| *fragment == "/chat?riceSchemaResult=ok" || fragment.starts_with("/chat?riceSchemaResult=failed&stage=")) {
                        println!("RICE_SCHEMA_RESULT {}", serde_json::json!({"verified":fragment == "/chat?riceSchemaResult=ok", "observedUrl":url.as_str()}));
                        std::io::stdout().flush().expect("flush schema result");
                        // Exercise the registered production exit command and
                        // its best-effort window-position save, not a kill.
                        window.eval("void window.__TAURI_INTERNALS__.invoke('app_exit')").expect("production exit IPC");
                        return;
                    }
                    if Instant::now() > deadline {
                        println!("RICE_SCHEMA_RESULT {}", serde_json::json!({"verified":false,"observedUrl":url.as_str()}));
                        std::io::stdout().flush().expect("flush schema timeout");
                        window.app_handle().exit(1);
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            });
            println!("RICE_NATIVE_READY {}", serde_json::json!({"hwnd":hwnd,"settingsPath":app.path().app_data_dir().expect("app data").join("settings.json")}));
            std::io::stdout().flush().expect("flush schema readiness");
            return;
        }
        let mut settings = app.state::<AppState>().settings.lock().expect("settings").clone();
        settings.speech.blocked_words = vec!["preserve-native-owner".into()];
        let launcher_budget = std::env::var_os("RICE_LAUNCHER_NATIVE_BUDGET").is_some();
        if launcher_budget {
            settings = crate::launcher::bounds_tests::full_quota_settings();
        }
        SettingsStore::save(app, &settings).expect("production owned save");
        if launcher_budget {
            *app.state::<AppState>().settings.lock().expect("publish fixture settings") = settings;
            let handle = app.clone();
            std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(60);
                loop {
                    let record = handle.state::<AppState>().settings.lock().expect("result settings").speech.blocked_words.first().cloned().unwrap_or_default();
                    if record.starts_with("RICE_LAUNCHER_RESULT ") {
                        println!("{record}");
                        std::io::stdout().flush().expect("flush measured result");
                        return;
                    }
                    if Instant::now() > deadline {
                        println!("RICE_LAUNCHER_RESULT {{\"error\":\"native result timed out\"}}");
                        std::io::stdout().flush().expect("flush timeout");
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            });
        }
        println!("RICE_NATIVE_READY {}", serde_json::json!({
            "hwnd": hwnd, "settingsPath": app.path().app_data_dir().expect("app data").join("settings.json")
        }));
        std::io::stdout().flush().expect("flush readiness");
    });
}

struct NativeChild(Child);
impl Drop for NativeChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_until(mut predicate: impl FnMut() -> bool, message: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !predicate() {
        assert!(Instant::now() < deadline, "{message}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn remove_fixture_directory(root: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match std::fs::remove_dir_all(root) {
            Ok(()) => return,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            // WebView2 browser processes release the isolated user-data folder
            // asynchronously after the host closes. Retry only Windows sharing,
            // lock and nonempty-directory races, and never hide a cleanup failure.
            Err(error)
                if matches!(error.raw_os_error(), Some(32 | 33 | 145))
                    && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => panic!("remove exact isolated fixture directory: {error}"),
        }
    }
}

#[test]
#[ignore = "requires a Windows desktop and WebView2; explicitly run by single-instance CI"]
fn native_two_process_restore_and_focus() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "rice-native-instance-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("isolated app directories");
    let id = format!("dev.rice.tests.instance{}", std::process::id());
    let spawn = |role: &str| {
        NativeChild(
            Command::new(std::env::current_exe().expect("test binary"))
                .args(["--exact", FIXTURE, "--nocapture"])
                .env("RICE_NATIVE_TEST_ID", &id)
                .env("RICE_NATIVE_TEST_ROLE", role)
                .env("RICE_NATIVE_TEST_ROOT", &root)
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("native child"),
        )
    };
    let mut owner = spawn("owner");
    let stdout = owner.0.stdout.take().expect("owner stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if let Some((_, record)) = line
                .expect("native child output")
                .split_once("RICE_NATIVE_READY ")
            {
                let _ = tx.send(
                    serde_json::from_str::<serde_json::Value>(record).expect("readiness record"),
                );
            }
        }
    });
    let ready = rx
        .recv_timeout(Duration::from_secs(60))
        .expect("real app readiness (WebView2 required)");
    let hwnd = ready["hwnd"].as_u64().expect("HWND") as usize as *mut std::ffi::c_void;
    let settings_path =
        std::path::PathBuf::from(ready["settingsPath"].as_str().expect("settings path"));
    assert!(
        settings_path.starts_with(&root),
        "fixture never writes real user settings"
    );
    let before = std::fs::read(&settings_path).expect("owner settings");
    // HWND belongs to the fixture process. Native calls do not alter another app.
    unsafe {
        ShowWindow(hwnd, SW_MINIMIZE);
    }
    wait_until(|| unsafe { IsIconic(hwnd) != 0 }, "owner minimized");
    let mut contender = spawn("contender");
    let mut status = None;
    wait_until(
        || {
            status = contender.0.try_wait().expect("contender status");
            status.is_some()
        },
        "second process exits",
    );
    assert!(
        status.expect("exit status").success(),
        "second launch exits normally in plugin"
    );
    wait_until(
        || unsafe { IsIconic(hwnd) == 0 && GetForegroundWindow() == hwnd },
        "production callback restores and focuses owner",
    );
    assert!(
        owner.0.try_wait().expect("owner status").is_none(),
        "original owner stays running"
    );
    assert_eq!(
        std::fs::read(&settings_path).expect("settings"),
        before,
        "second launch never writes stale settings"
    );
    // Let the production window/WebView shutdown path release browser storage,
    // rather than abruptly killing its host after successful assertions.
    assert_ne!(unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) }, 0);
    let mut owner_status = None;
    wait_until(
        || {
            owner_status = owner.0.try_wait().expect("owner shutdown status");
            owner_status.is_some()
        },
        "owner closes normally",
    );
    assert!(owner_status.expect("owner exit status").success());
    drop(contender);
    drop(owner);
    remove_fixture_directory(&root);
}

#[test]
#[ignore = "requires a Windows desktop and WebView2; explicitly run by native CI"]
fn native_future_settings_preserved_through_ipc_and_exit() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("rice-native-schema-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut owner = NativeChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", FIXTURE, "--nocapture"])
            .env(
                "RICE_NATIVE_TEST_ID",
                format!("dev.rice.tests.schema{}", std::process::id()),
            )
            .env("RICE_NATIVE_TEST_ROLE", "owner")
            .env("RICE_NATIVE_TEST_ROOT", &root)
            .env("RICE_SETTINGS_SCHEMA_NATIVE", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let stdout = owner.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let line = line.unwrap();
            for prefix in ["RICE_NATIVE_READY ", "RICE_SCHEMA_RESULT "] {
                if let Some((_, record)) = line.split_once(prefix) {
                    tx.send((
                        prefix,
                        serde_json::from_str::<serde_json::Value>(record).unwrap(),
                    ))
                    .unwrap();
                }
            }
        }
    });
    let mut ready = None;
    let mut result = None;
    for _ in 0..2 {
        let (prefix, record) = rx
            .recv_timeout(Duration::from_secs(75))
            .expect("native schema requires desktop/WebView2");
        if prefix == "RICE_NATIVE_READY " {
            ready = Some(record);
        } else {
            result = Some(record);
        }
    }
    println!(
        "Native settings schema: {}",
        result.as_ref().expect("IPC result")
    );
    let mut status = None;
    wait_until(
        || {
            status = owner.0.try_wait().unwrap();
            status.is_some()
        },
        "production app_exit closes normally despite read-only settings",
    );
    assert!(status.unwrap().success());
    let ready = ready.expect("production startup");
    let path = std::path::Path::new(ready["settingsPath"].as_str().unwrap());
    assert!(
        path.starts_with(&root),
        "fixture remains inside isolated app-data"
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), FUTURE_SETTINGS);
    assert_eq!(
        std::fs::read_to_string(path.with_file_name("settings.json.bak")).unwrap(),
        FUTURE_BACKUP
    );
    let mut names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["settings.json", "settings.json.bak", "settings.writer.lock"],
        "no temporary or quarantine files"
    );
    drop(owner);
    remove_fixture_directory(&root);
    assert_eq!(
        result.expect("production IPC verification")["verified"],
        true
    );
}

#[test]
#[ignore = "requires a Windows desktop and WebView2; explicitly run by native CI"]
fn native_maximum_launcher_render_and_ipc_budget() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "rice-native-launcher-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut owner = NativeChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", FIXTURE, "--nocapture"])
            .env(
                "RICE_NATIVE_TEST_ID",
                format!("dev.rice.tests.launcher{}", std::process::id()),
            )
            .env("RICE_NATIVE_TEST_ROLE", "owner")
            .env("RICE_NATIVE_TEST_ROOT", &root)
            .env("RICE_LAUNCHER_NATIVE_BUDGET", "1")
            // Diagnostic precision flag is confined to this isolated test child.
            .env(
                "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
                "--enable-precise-memory-info",
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let stdout = owner.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let line = line.unwrap();
            for prefix in ["RICE_NATIVE_READY ", "RICE_LAUNCHER_RESULT "] {
                if let Some((_, record)) = line.split_once(prefix) {
                    tx.send((
                        prefix,
                        serde_json::from_str::<serde_json::Value>(record).unwrap(),
                    ))
                    .unwrap();
                }
            }
        }
    });
    let mut ready = None;
    let mut result = None;
    for _ in 0..2 {
        let (prefix, record) = rx
            .recv_timeout(Duration::from_secs(75))
            .expect("native measurement requires desktop/WebView2");
        if prefix == "RICE_NATIVE_READY " {
            ready = Some(record);
        } else {
            result = Some(record);
        }
    }
    let ready = ready.expect("real app readiness");
    let result = result.expect("real IPC/UI result");
    println!("Native Launcher budget: {result}");
    let hwnd = ready["hwnd"].as_u64().unwrap() as usize as *mut std::ffi::c_void;
    assert_ne!(unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) }, 0);
    let mut status = None;
    wait_until(
        || {
            status = owner.0.try_wait().unwrap();
            status.is_some()
        },
        "fixture closes normally",
    );
    assert!(status.unwrap().success());
    drop(owner);
    remove_fixture_directory(&root);
    assert!(result.get("error").is_none(), "{result}");
    assert_eq!(result["count"], 200);
    assert_eq!(result["rejected"], 4);
    assert_eq!(result["validationRejected"], 6);
    assert_eq!(result["remoteRejected"], 2);
    assert_eq!(result["unchanged"], true);
    assert_eq!(result["styleChecks"], 16);
    assert!(
        result["getMs"].as_f64().unwrap() < 2000.0,
        "full settings IPC retrieval within 2 seconds: {result}"
    );
    assert!(
        result["renderMs"].as_f64().unwrap() < 2000.0,
        "200 tiles/icons render within 2 seconds: {result}"
    );
    assert!(
        result["incrementalJsHeap"].as_u64().unwrap() > 0,
        "heap sampling must observe real allocation, not a stale zero delta: {result}"
    );
    assert!(
        result["incrementalJsHeap"].as_u64().unwrap() <= 64 * 1024 * 1024,
        "renderer JS live heap delta within 64MiB (not full WebView RSS/GPU): {result}"
    );
}
