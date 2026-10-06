//! IPC conversion and composition only. Use cases and log policy live elsewhere.
use super::events::AppEventSink;
use super::model::{parse_add_request, LauncherAddResult, LauncherItem, LauncherLaunchResult};
use super::repository::AppSettingsRepository;
use crate::application::AppState;
use crate::settings::{AppSettings, SettingsStore};

fn repository<'a>(
    app: &'a tauri::AppHandle<tauri::Wry>,
    state: &'a AppState,
) -> AppSettingsRepository<'a, impl Fn(&AppSettings) -> Result<(), String> + Send + Sync + 'a> {
    AppSettingsRepository {
        settings: &state.settings,
        transaction: &state.settings_transaction,
        persist: move |candidate: &AppSettings| {
            SettingsStore::save(app, candidate)
                .map_err(|error| format!("ランチャーの設定を保存できませんでした: {error}"))
        },
    }
}

#[tauri::command]
pub async fn launcher_add(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<LauncherAddResult, String> {
    let paths = parse_add_request(crate::resource_limits::request_json(&request)?)?;
    state
        .launcher_runtime
        .service(&repository(&app, &state), &AppEventSink(&app))
        .add(paths)
        .await
}

#[tauri::command]
pub fn launcher_remove(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
    item_id: String,
) -> Result<Vec<LauncherItem>, String> {
    state
        .launcher_runtime
        .service(&repository(&app, &state), &AppEventSink(&app))
        .remove(&item_id)
}

#[tauri::command]
pub async fn launcher_launch(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
    item_id: String,
) -> Result<LauncherLaunchResult, String> {
    Ok(state
        .launcher_runtime
        .service(&repository(&app, &state), &AppEventSink(&app))
        .launch(&item_id)
        .await)
}

#[tauri::command]
pub async fn launcher_launch_all(
    app: tauri::AppHandle<tauri::Wry>,
    state: tauri::State<'_, AppState>,
) -> Result<LauncherLaunchResult, String> {
    Ok(state
        .launcher_runtime
        .service(&repository(&app, &state), &AppEventSink(&app))
        .launch_all()
        .await)
}
