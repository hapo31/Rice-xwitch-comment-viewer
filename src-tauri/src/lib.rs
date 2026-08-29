mod app_events;
mod launcher;
mod settings;
mod speech;
mod twitch;

#[cfg(feature = "app")]
use app_events::{
    app_events_snapshot, emit_app_log, emit_speech_status, emit_twitch_auth_required,
    emit_twitch_status, AppEventState, AppLogLevel, SpeechStatus, TwitchAuthRequiredReason,
    TwitchStatus, TwitchStatusDomain,
};
#[cfg(feature = "app")]
use launcher::{launcher_add, launcher_launch, launcher_launch_all, launcher_remove};
use serde::Serialize;
#[cfg(feature = "app")]
use settings::{
    settings_get, settings_take_recovery_notice, settings_update, AppSettings, AppState,
    SettingsStore, WindowPosition,
};
#[cfg(feature = "app")]
use speech::bouyomi::{
    speech_clear, speech_connection_diagnostics, speech_health_check, speech_health_probe,
    speech_pause, speech_resume, speech_skip, speech_test,
};
#[cfg(feature = "app")]
use speech::{
    emit_current_queue, speech_queue_dismiss, speech_queue_dismiss_history, speech_queue_reload,
    speech_queue_remove, speech_queue_retry,
};
#[cfg(feature = "app")]
use std::process::Command;
use std::sync::Mutex;
#[cfg(feature = "app")]
use tauri::{Manager, PhysicalPosition, WindowEvent};
#[cfg(feature = "app")]
use twitch::{
    twitch_connect, twitch_disconnect, twitch_get_stored_auth, twitch_poll_auth, twitch_start_auth,
    twitch_stop_chat, twitch_validate_auth, TwitchAuthStore,
};

#[cfg(feature = "app")]
#[tauri::command]
fn app_exit(app: tauri::AppHandle) {
    persist_main_window_position(&app);
    app.exit(0);
}

#[cfg(feature = "app")]
#[tauri::command]
fn app_open_external_url(url: String) -> Result<(), String> {
    let url = validate_external_url(&url)?;
    open_external_url(url.as_str()).map_err(|error| {
        format!("ブラウザを開けませんでした。URLをコピーして手動で開いてください: {error}")
    })
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct AppBuildInfo {
    version: &'static str,
    is_dev: bool,
    commit_hash: Option<&'static str>,
}

fn app_build_info_value() -> AppBuildInfo {
    AppBuildInfo {
        version: env!("CARGO_PKG_VERSION"),
        is_dev: cfg!(debug_assertions),
        commit_hash: option_env!("RICE_GIT_COMMIT"),
    }
}

#[cfg(feature = "app")]
#[tauri::command]
fn app_build_info() -> AppBuildInfo {
    app_build_info_value()
}

#[cfg(feature = "app")]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .manage(AppEventState::default())
        .invoke_handler(tauri::generate_handler![
            app_exit,
            app_open_external_url,
            app_build_info,
            app_events_snapshot,
            launcher_add,
            launcher_remove,
            launcher_launch,
            launcher_launch_all,
            settings_get,
            settings_take_recovery_notice,
            settings_update,
            speech_health_check,
            speech_health_probe,
            speech_connection_diagnostics,
            speech_test,
            speech_pause,
            speech_resume,
            speech_skip,
            speech_clear,
            speech_queue_reload,
            speech_queue_remove,
            speech_queue_dismiss,
            speech_queue_dismiss_history,
            speech_queue_retry,
            twitch_start_auth,
            twitch_poll_auth,
            twitch_validate_auth,
            twitch_connect,
            twitch_stop_chat,
            twitch_get_stored_auth,
            twitch_disconnect
        ])
        .setup(|app| {
            let state = app.state::<AppState>();
            let loaded_settings = settings::SettingsStore::load(app.handle())?;
            restore_main_window_position(app.handle(), &loaded_settings.settings);
            *state.settings.lock().expect("settings mutex poisoned") = loaded_settings.settings;
            *state
                .settings_recovery_notice
                .lock()
                .expect("settings recovery mutex poisoned") = loaded_settings.recovery_notice;
            let recovery_message = state
                .settings_recovery_notice
                .lock()
                .expect("settings recovery mutex poisoned")
                .as_ref()
                .map(|notice| notice.message.clone());
            if let Some(message) = recovery_message {
                emit_app_log(app.handle(), AppLogLevel::Warning, message);
            } else {
                emit_app_log(app.handle(), AppLogLevel::Info, "設定を読み込みました。");
            }
            emit_twitch_status(
                app.handle(),
                TwitchStatusDomain::Chat,
                TwitchStatus::Disconnected,
                Some("Twitch は未接続です。".to_string()),
            );
            emit_speech_status(
                app.handle(),
                SpeechStatus::Disconnected,
                Some("棒読みちゃん接続を確認してください。".to_string()),
            );
            if let Err(error) = emit_current_queue(app.handle()) {
                emit_app_log(app.handle(), AppLogLevel::Error, error);
            }
            let restored_auth = TwitchAuthStore::load();
            let has_restored_auth = restored_auth.auth.is_some();
            if let Some(auth) = restored_auth.auth {
                *state
                    .twitch_auth
                    .lock()
                    .expect("twitch auth mutex poisoned") = auth;
                emit_twitch_status(
                    app.handle(),
                    TwitchStatusDomain::Auth,
                    TwitchStatus::Validating,
                    Some("保存済みの Twitch 認証情報を復元しました。検証しています。".to_string()),
                );
                emit_app_log(
                    app.handle(),
                    AppLogLevel::Info,
                    "保存済みの Twitch 認証情報を復元しました。/validate を実行して確認します。",
                );
            }
            if let Some(warning) = restored_auth.storage_warning {
                emit_app_log(app.handle(), AppLogLevel::Warning, warning.clone());
                if !has_restored_auth && warning.contains("Twitch 認証に必要な権限がありません")
                {
                    emit_twitch_auth_required(
                        app.handle(),
                        TwitchAuthRequiredReason::MissingRequiredScope,
                        warning,
                    );
                } else {
                    emit_twitch_status(
                        app.handle(),
                        TwitchStatusDomain::Auth,
                        if has_restored_auth {
                            TwitchStatus::Validating
                        } else {
                            TwitchStatus::AuthRequired
                        },
                        Some(warning),
                    );
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, WindowEvent::CloseRequested { .. }) {
                persist_main_window_position(window.app_handle());
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(feature = "app")]
fn restore_main_window_position(app: &tauri::AppHandle, settings: &AppSettings) {
    let Some(position) = settings.window.position else {
        return;
    };
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let Ok(monitors) = window.available_monitors() else {
        return;
    };

    let visible = monitors.iter().any(|monitor| {
        let area = monitor.work_area();
        title_bar_is_visible(
            position,
            size.width,
            area.position.x,
            area.position.y,
            area.size.width,
            area.size.height,
        )
    });
    if visible {
        let _ = window.set_position(PhysicalPosition::new(position.x, position.y));
    }
}

#[cfg(feature = "app")]
fn persist_main_window_position(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_minimized().unwrap_or(false) {
        return;
    }
    let Ok(position) = window.outer_position() else {
        return;
    };
    let state = app.state::<AppState>();
    let Ok(mut settings) = state.settings.lock() else {
        return;
    };

    let mut candidate = settings.clone();
    candidate.window.position = Some(WindowPosition {
        x: position.x,
        y: position.y,
    });
    if SettingsStore::save(app, &candidate).is_ok() {
        *settings = candidate;
    }
}

#[cfg(feature = "app")]
const MIN_VISIBLE_WINDOW_WIDTH_PX: i64 = 64;
#[cfg(feature = "app")]
const MIN_VISIBLE_WINDOW_HEIGHT_PX: i64 = 32;

#[cfg(feature = "app")]
fn title_bar_is_visible(
    position: WindowPosition,
    window_width: u32,
    work_area_x: i32,
    work_area_y: i32,
    work_area_width: u32,
    work_area_height: u32,
) -> bool {
    let window_left = i64::from(position.x);
    let window_top = i64::from(position.y);
    let window_right = window_left + i64::from(window_width);
    let title_bar_bottom = window_top + MIN_VISIBLE_WINDOW_HEIGHT_PX;
    let area_left = i64::from(work_area_x);
    let area_top = i64::from(work_area_y);
    let area_right = area_left + i64::from(work_area_width);
    let area_bottom = area_top + i64::from(work_area_height);

    let visible_width = (window_right.min(area_right) - window_left.max(area_left)).max(0);
    let visible_height = (title_bar_bottom.min(area_bottom) - window_top.max(area_top)).max(0);

    visible_width >= MIN_VISIBLE_WINDOW_WIDTH_PX && visible_height >= MIN_VISIBLE_WINDOW_HEIGHT_PX
}

#[cfg(feature = "app")]
fn validate_external_url(raw_url: &str) -> Result<reqwest::Url, String> {
    let url =
        reqwest::Url::parse(raw_url).map_err(|_| "外部ブラウザで開けないURLです。".to_string())?;
    let host = url
        .host_str()
        .ok_or_else(|| "外部ブラウザで開けないURLです。".to_string())?;

    if url.scheme() == "https"
        && matches!(host, "www.twitch.tv" | "twitch.tv")
        && url.path() == "/activate"
    {
        Ok(url)
    } else {
        Err("許可されていない外部URLです。".to_string())
    }
}

#[cfg(feature = "app")]
fn open_external_url(url: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        return run_open_command("rundll32", &["url.dll,FileProtocolHandler", url]);
    }

    #[cfg(target_os = "macos")]
    {
        return run_open_command("open", &[url]);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let commands: &[(&str, &[&str])] = if is_wsl() {
            &[
                ("wslview", &[url]),
                ("xdg-open", &[url]),
                ("gio", &["open", url]),
            ]
        } else {
            &[
                ("xdg-open", &[url]),
                ("gio", &["open", url]),
                ("wslview", &[url]),
            ]
        };

        let mut errors = Vec::new();
        for (program, args) in commands {
            match run_open_command(program, args) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(format!("{program}: {error}")),
            }
        }

        Err(anyhow::anyhow!(errors.join("; ")))
    }
}

#[cfg(feature = "app")]
fn run_open_command(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new(program).args(args).output()?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(anyhow::anyhow!("終了コード {}", output.status))
    } else {
        Err(anyhow::anyhow!(stderr))
    }
}

#[cfg(all(feature = "app", unix, not(target_os = "macos")))]
fn is_wsl() -> bool {
    std::env::var_os("WSL_DISTRO_NAME").is_some()
        || std::fs::read_to_string("/proc/version")
            .map(|version| version.to_ascii_lowercase().contains("microsoft"))
            .unwrap_or(false)
}

pub(crate) type SharedSettings<T> = Mutex<T>;

#[cfg(all(test, feature = "app"))]
mod tests {
    use super::{app_build_info_value, title_bar_is_visible, validate_external_url};
    use crate::settings::WindowPosition;

    #[test]
    fn reports_package_build_information() {
        let info = app_build_info_value();

        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.is_dev, cfg!(debug_assertions));
        assert!(info.commit_hash.is_none_or(
            |hash| hash.len() == 7 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        ));
    }

    #[test]
    fn allows_twitch_activate_url() {
        assert!(validate_external_url("https://www.twitch.tv/activate").is_ok());
        assert!(validate_external_url("https://twitch.tv/activate").is_ok());
    }

    #[test]
    fn rejects_untrusted_external_url() {
        assert!(validate_external_url("https://example.com/activate").is_err());
        assert!(validate_external_url("http://www.twitch.tv/activate").is_err());
        assert!(validate_external_url("https://www.twitch.tv/settings").is_err());
    }

    #[test]
    fn keeps_a_saved_position_when_part_of_the_window_is_still_visible() {
        assert!(title_bar_is_visible(
            WindowPosition { x: -900, y: 120 },
            1180,
            0,
            0,
            1920,
            1080,
        ));
    }

    #[test]
    fn skips_a_saved_position_that_is_outside_the_current_monitor_layout() {
        assert!(!title_bar_is_visible(
            WindowPosition { x: 5000, y: 120 },
            1180,
            0,
            0,
            1920,
            1080,
        ));
    }

    #[test]
    fn requires_a_recoverable_title_bar_area_to_be_visible() {
        assert!(!title_bar_is_visible(
            WindowPosition { x: 1860, y: 1055 },
            1180,
            0,
            0,
            1920,
            1080,
        ));
    }

    #[test]
    fn skips_a_window_whose_only_visible_area_is_below_an_removed_monitor() {
        assert!(!title_bar_is_visible(
            WindowPosition { x: 0, y: -728 },
            1180,
            0,
            0,
            1920,
            1080,
        ));
    }
}
