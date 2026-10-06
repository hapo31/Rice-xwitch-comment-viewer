mod app_events;
#[cfg(target_os = "linux")]
mod external_url;
mod launcher;
mod resource_limits;
mod settings;
#[cfg(feature = "app")]
mod single_instance;
mod speech;
mod twitch;
#[cfg(all(test, feature = "app"))]
mod twitch_test_ports;
#[cfg(test)]
mod wire_contracts;

#[cfg(feature = "app")]
use app_events::{
    app_events_snapshot, emit_app_log, emit_speech_adapter_health, emit_twitch_auth_required,
    emit_twitch_status, AppEventState, AppLogLevel, TwitchAuthRequiredReason, TwitchStatus,
    TwitchStatusDomain,
};
#[cfg(feature = "app")]
use launcher::commands::{launcher_add, launcher_launch, launcher_launch_all, launcher_remove};
use serde::Serialize;
#[cfg(feature = "app")]
use settings::{
    settings_get, settings_take_recovery_notice, settings_update, AppSettings, AppState,
    SettingsStore, WindowPosition,
};
#[cfg(feature = "app")]
use speech::bouyomi::speech_connection_diagnostics;
#[cfg(feature = "app")]
use speech::commands::{
    speech_clear, speech_health_check, speech_health_probe, speech_pause, speech_resume,
    speech_skip, speech_test,
};
#[cfg(feature = "app")]
use speech::destination::speech_authorize_endpoint;
#[cfg(feature = "app")]
use speech::{
    emit_current_queue, speech_queue_dismiss, speech_queue_dismiss_history, speech_queue_reload,
    speech_queue_remove, speech_queue_retry,
};
use std::sync::Mutex;
#[cfg(feature = "app")]
use tauri::{Manager, PhysicalPosition, WindowEvent};
#[cfg(all(feature = "app", not(target_os = "linux")))]
use tauri_plugin_opener::OpenerExt;
#[cfg(feature = "app")]
use twitch::commands::{
    twitch_connect, twitch_disconnect, twitch_get_stored_auth, twitch_poll_auth, twitch_start_auth,
    twitch_stop_chat, twitch_validate_auth,
};

#[cfg(feature = "app")]
#[tauri::command]
fn app_exit(app: tauri::AppHandle) {
    persist_main_window_position(&app);
    app.exit(0);
}

#[cfg(feature = "app")]
#[tauri::command]
fn app_open_external_url(_app: tauri::AppHandle, url: String) -> Result<(), String> {
    open_validated_external_url(&url, |url| {
        #[cfg(target_os = "linux")]
        {
            external_url::open_system_url(url.as_str())
        }
        #[cfg(not(target_os = "linux"))]
        {
            open_system_url(&_app, url.as_str())
        }
    })
}

#[cfg(all(feature = "app", not(target_os = "linux")))]
fn open_system_url(app: &tauri::AppHandle, url: &str) -> anyhow::Result<()> {
    app.opener()
        .open_url(url.to_string(), None::<String>)
        .map_err(Into::into)
}

#[cfg(feature = "app")]
fn open_validated_external_url<E: std::fmt::Display>(
    raw_url: &str,
    open: impl FnOnce(&reqwest::Url) -> Result<(), E>,
) -> Result<(), String> {
    let url = validate_external_url(raw_url)?;
    open(&url).map_err(external_url_open_error)
}

#[cfg(feature = "app")]
fn external_url_open_error(error: impl std::fmt::Display) -> String {
    format!("ブラウザを開けませんでした。URLをコピーして手動で開いてください: {error}")
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
#[cfg_attr(test, ts(optional_fields))]
struct AppBuildInfo {
    version: &'static str,
    is_dev: bool,
    launcher: launcher::LauncherCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_hash: Option<&'static str>,
}

fn app_build_info_value() -> AppBuildInfo {
    AppBuildInfo {
        version: env!("CARGO_PKG_VERSION"),
        is_dev: cfg!(debug_assertions),
        launcher: launcher::LauncherCapabilities::current(),
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
    app_builder()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(feature = "app")]
fn app_builder() -> tauri::Builder<tauri::Wry> {
    app_builder_with_state(AppState::default())
}

#[cfg(feature = "app")]
fn app_builder_with_state(state: AppState) -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        // The ownership plugin must be first, before other plugins and setup.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            single_instance::request_activation(app);
        }))
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .manage(single_instance::PendingActivation::default())
        .manage(state)
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
            speech_authorize_endpoint,
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
            emit_speech_adapter_health(
                app.handle(),
                app_events::SpeechAdapterHealth::Unknown,
                Some("棒読みちゃん接続を確認してください。".to_string()),
            );
            if let Err(error) = emit_current_queue(app.handle()) {
                emit_app_log(app.handle(), AppLogLevel::Error, error);
            }
            let restored_auth = tauri::async_runtime::block_on(state.twitch_auth_store.load())?;
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
            if let Some(notice) = restored_auth.notice {
                let (status, reason) = restored_auth_status(has_restored_auth, &notice);
                emit_app_log(app.handle(), AppLogLevel::Warning, notice.message.clone());
                if let Some(reason) = reason {
                    emit_twitch_auth_required(app.handle(), reason, notice.message);
                } else {
                    emit_twitch_status(
                        app.handle(),
                        TwitchStatusDomain::Auth,
                        status,
                        Some(notice.message),
                    );
                }
            }
            single_instance::mark_ready(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, WindowEvent::CloseRequested { .. }) {
                persist_main_window_position(window.app_handle());
            }
        })
}

#[cfg(feature = "app")]
fn restored_auth_status(
    has_restored_auth: bool,
    notice: &twitch::AuthLoadNotice,
) -> (TwitchStatus, Option<TwitchAuthRequiredReason>) {
    match (has_restored_auth, notice.reason) {
        (true, _) => (TwitchStatus::Validating, None),
        (false, twitch::AuthLoadReason::MissingRequiredScope) => (
            TwitchStatus::AuthRequired,
            Some(TwitchAuthRequiredReason::MissingRequiredScope),
        ),
        (false, _) => (TwitchStatus::AuthRequired, None),
    }
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
    let Ok(_transaction) = state.settings_transaction.lock() else {
        return;
    };
    let Ok(mut candidate) = state.settings.lock().map(|settings| settings.clone()) else {
        return;
    };
    candidate.window.position = Some(WindowPosition {
        x: position.x,
        y: position.y,
    });
    if SettingsStore::save(app, &candidate).is_ok() {
        if let Ok(mut settings) = state.settings.lock() {
            *settings = candidate;
        }
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

pub(crate) type SharedSettings<T> = Mutex<T>;

#[cfg(all(test, feature = "app"))]
mod tests {
    use super::{
        app_build_info_value, external_url_open_error, open_validated_external_url,
        title_bar_is_visible, validate_external_url,
    };
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
    fn external_browser_failure_keeps_japanese_recovery_guidance() {
        let message = external_url_open_error("system opener failed");

        assert!(
            message.starts_with("ブラウザを開けませんでした。URLをコピーして手動で開いてください:")
        );
        assert!(message.contains("system opener failed"));
    }

    #[test]
    fn rejects_untrusted_url_before_calling_the_opener() {
        let mut opener_called = false;

        let result = open_validated_external_url("https://example.com/activate", |_| {
            opener_called = true;
            Ok::<(), std::convert::Infallible>(())
        });

        assert_eq!(result, Err("許可されていない外部URLです。".to_string()));
        assert!(!opener_called);
    }

    #[test]
    fn opens_the_validated_twitch_activation_url() {
        let mut opened_url = None;

        let result =
            open_validated_external_url("https://www.twitch.tv/activate?device-code=123", |url| {
                opened_url = Some(url.to_string());
                Ok::<(), std::convert::Infallible>(())
            });

        assert_eq!(result, Ok(()));
        assert_eq!(
            opened_url.as_deref(),
            Some("https://www.twitch.tv/activate?device-code=123")
        );
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

#[cfg(all(test, feature = "app"))]
#[test]
fn auth_restore_notification_uses_reason_even_when_display_text_changes() {
    use twitch::{AuthLoadNotice, AuthLoadReason};
    for message in [
        "文言を変更しました。",
        "Twitch 認証に必要な権限がありません",
    ] {
        for reason in [
            AuthLoadReason::MissingRequiredScope,
            AuthLoadReason::StoreUnavailable,
            AuthLoadReason::CorruptData,
            AuthLoadReason::LegacyMigrated,
            AuthLoadReason::LegacyCleanupFailed,
        ] {
            let notice = AuthLoadNotice {
                reason,
                message: message.into(),
            };
            let (status, required_reason) = restored_auth_status(false, &notice);
            assert!(matches!(status, TwitchStatus::AuthRequired));
            assert_eq!(
                required_reason.is_some(),
                reason == AuthLoadReason::MissingRequiredScope
            );
            let (status, required_reason) = restored_auth_status(true, &notice);
            assert!(matches!(status, TwitchStatus::Validating));
            assert!(required_reason.is_none());
        }
    }
}
