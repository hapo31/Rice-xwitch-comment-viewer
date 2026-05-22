mod app_events;
mod settings;
mod speech;
mod twitch;

#[cfg(feature = "app")]
use settings::{settings_get, settings_update, AppState};
#[cfg(feature = "app")]
use speech::bouyomi::{
    speech_clear, speech_connection_diagnostics, speech_health_check, speech_pause, speech_resume,
    speech_skip, speech_test,
};
use std::sync::Mutex;
#[cfg(feature = "app")]
use tauri::Manager;
#[cfg(feature = "app")]
use twitch::{
    twitch_disconnect, twitch_get_stored_auth, twitch_poll_auth, twitch_start_auth,
    twitch_validate_auth, TwitchAuthStore,
};

#[cfg(feature = "app")]
#[tauri::command]
fn app_exit(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg(feature = "app")]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            app_exit,
            settings_get,
            settings_update,
            speech_health_check,
            speech_connection_diagnostics,
            speech_test,
            speech_pause,
            speech_resume,
            speech_skip,
            speech_clear,
            twitch_start_auth,
            twitch_poll_auth,
            twitch_validate_auth,
            twitch_get_stored_auth,
            twitch_disconnect
        ])
        .setup(|app| {
            let state = app.state::<AppState>();
            let settings = settings::SettingsStore::load(app.handle())?;
            *state.settings.lock().expect("settings mutex poisoned") = settings;
            if let Ok(Some(auth)) = TwitchAuthStore::load() {
                *state
                    .twitch_auth
                    .lock()
                    .expect("twitch auth mutex poisoned") = auth;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub(crate) type SharedSettings<T> = Mutex<T>;
