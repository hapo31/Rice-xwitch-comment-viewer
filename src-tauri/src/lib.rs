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
#[cfg(feature = "app")]
use std::process::Command;
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
#[tauri::command]
fn app_open_external_url(url: String) -> Result<(), String> {
    let url = validate_external_url(&url)?;
    open_external_url(url.as_str()).map_err(|error| {
        format!("ブラウザを開けませんでした。URLをコピーして手動で開いてください: {error}")
    })
}

#[cfg(feature = "app")]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            app_exit,
            app_open_external_url,
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
    use super::validate_external_url;

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
}
