use crate::settings::{AppSettings, SettingsRecoveryNotice};
use crate::speech::SpeechQueueState;
use crate::twitch::TwitchAuthState;
#[cfg(feature = "app")]
use crate::twitch::{TwitchAuthStore, TwitchConnectionHandle};
use crate::SharedSettings;

#[derive(Default)]
pub struct AppState {
    pub settings: SharedSettings<AppSettings>,
    /// Shared launcher adapters and bounded worker pool, used by every command.
    pub launcher_runtime: crate::launcher::LauncherRuntime,
    pub settings_recovery_notice: SharedSettings<Option<SettingsRecoveryNotice>>,
    #[cfg(feature = "app")]
    pub twitch_auth: std::sync::Arc<std::sync::Mutex<TwitchAuthState>>,
    #[cfg(not(feature = "app"))]
    pub twitch_auth: SharedSettings<TwitchAuthState>,
    pub speech_queue: std::sync::Arc<std::sync::Mutex<SpeechQueueState>>,
    /// Shared selection, ordering and clock for every speech operation.
    pub speech_runtime: crate::speech::runtime::SpeechRuntime,
    #[cfg(feature = "app")]
    pub twitch_connection: SharedSettings<Option<TwitchConnectionHandle>>,
    #[cfg(feature = "app")]
    pub twitch_auth_store: TwitchAuthStore,
}
