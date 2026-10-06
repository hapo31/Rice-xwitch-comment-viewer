//! Stable Tauri adapters: service calls only, no HTTP or keyring implementation.
use super::auth_service::TwitchAuthService;
use super::auth_service::stored_auth_profile;
use super::auth_state::{
    TwitchAuthPollResult, TwitchAuthValidationResult, TwitchDeviceAuthStart, TwitchUserProfile,
};
use super::chat_service::TwitchChatService;
use super::runtime::TauriTwitchRuntime;
use crate::application::AppState;

#[tauri::command]
pub async fn twitch_start_auth(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<TwitchDeviceAuthStart, String> {
    TwitchAuthService::new(&TauriTwitchRuntime::new(&state, app))
        .start()
        .await
}

#[tauri::command]
pub async fn twitch_poll_auth(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<TwitchAuthPollResult, String> {
    TwitchAuthService::new(&TauriTwitchRuntime::new(&state, app))
        .poll()
        .await
}

#[tauri::command]
pub async fn twitch_validate_auth(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<TwitchAuthValidationResult, String> {
    TwitchAuthService::new(&TauriTwitchRuntime::new(&state, app))
        .validate()
        .await
}

#[tauri::command]
pub fn twitch_get_stored_auth(
    state: tauri::State<'_, AppState>,
) -> Result<Option<TwitchUserProfile>, String> {
    stored_auth_profile(&state.twitch_auth)
}

#[tauri::command]
pub async fn twitch_connect(
    channel_login: Option<String>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), crate::settings::validation::ValidationError> {
    TwitchChatService::new(TauriTwitchRuntime::new(&state, app))
        .connect(channel_login)
        .await
}

#[tauri::command]
pub async fn twitch_disconnect(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    TwitchChatService::new(TauriTwitchRuntime::new(&state, app))
        .disconnect()
        .await
}

#[tauri::command]
pub fn twitch_stop_chat(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    TwitchChatService::new(TauriTwitchRuntime::new(&state, app)).stop()
}
