use super::{
    SpeechAdapter, SpeechControl, SpeechFailure, SpeechFuture, SpeechHealth,
    SpeechPlaybackCompletion, SpeechRequest, SpeechResult,
};
use crate::settings::{AppState, SpeechSettings};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub type SpeechDispatcher = Arc<tokio::sync::Mutex<()>>;

/// The only place that selects an implementation from a settings snapshot.
pub trait SpeechAdapterFactory: Send + Sync {
    fn select(
        &self,
        settings: &SpeechSettings,
        dispatcher: SpeechDispatcher,
    ) -> Result<Arc<dyn SpeechAdapter>, SpeechFailure>;
    fn connection_confirmation(&self, settings: &SpeechSettings) -> Option<String> {
        settings.connection_success_speech_enabled.then(|| {
            let text = settings.connection_success_speech_text.trim();
            if text.is_empty() {
                "読み上げ先と接続しました".to_string()
            } else {
                text.to_string()
            }
        })
    }
}

pub trait SpeechClock: Send + Sync {
    fn now(&self) -> Instant;
    fn sleep(&self, delay: Duration) -> SpeechFuture<'_, ()>;
}

pub struct SystemSpeechClock;
impl SpeechClock for SystemSpeechClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn sleep(&self, delay: Duration) -> SpeechFuture<'_, ()> {
        Box::pin(tokio::time::sleep(delay))
    }
}

#[derive(Clone)]
pub struct SpeechRuntime {
    dispatcher: SpeechDispatcher,
    factory: Arc<dyn SpeechAdapterFactory>,
    pub(crate) clock: Arc<dyn SpeechClock>,
}

impl Default for SpeechRuntime {
    fn default() -> Self {
        Self::new(
            Arc::new(super::factory::ConfiguredAdapterFactory),
            Arc::new(SystemSpeechClock),
        )
    }
}

impl SpeechRuntime {
    pub fn new(factory: Arc<dyn SpeechAdapterFactory>, clock: Arc<dyn SpeechClock>) -> Self {
        Self {
            dispatcher: SpeechDispatcher::default(),
            factory,
            clock,
        }
    }

    pub fn select(
        &self,
        settings: &SpeechSettings,
    ) -> Result<SelectedSpeechAdapter, SpeechFailure> {
        Ok(SelectedSpeechAdapter {
            adapter: self.factory.select(settings, self.dispatcher.clone())?,
            dispatcher: self.dispatcher.clone(),
            confirmation_text: self.factory.connection_confirmation(settings),
        })
    }

    pub(crate) fn select_from_state(
        &self,
        state: &AppState,
    ) -> Result<SelectedSpeechAdapter, SpeechFailure> {
        let snapshot = state
            .settings
            .lock()
            .map_err(|error| SpeechFailure::unknown(error.to_string()))?
            .speech
            .clone();
        self.select(&snapshot)
    }

    pub(crate) fn dispatcher(&self) -> SpeechDispatcher {
        self.dispatcher.clone()
    }
}

/// Keeps raw adapter calls behind a common ordering gate. Holding this session
/// spans request reservation OR remote control + its local application/events.
#[derive(Clone)]
pub struct SelectedSpeechAdapter {
    adapter: Arc<dyn SpeechAdapter>,
    dispatcher: SpeechDispatcher,
    pub(crate) confirmation_text: Option<String>,
}

impl SelectedSpeechAdapter {
    pub async fn lock(&self) -> SpeechSession {
        SpeechSession {
            adapter: self.adapter.clone(),
            _guard: self.dispatcher.clone().lock_owned().await,
        }
    }
    pub async fn wait_for_completion(&self) -> SpeechPlaybackCompletion {
        self.adapter.wait_for_completion().await
    }
    pub(crate) fn session_after_dispatch_lock(
        &self,
        guard: tokio::sync::OwnedMutexGuard<()>,
    ) -> SpeechSession {
        SpeechSession {
            adapter: self.adapter.clone(),
            _guard: guard,
        }
    }
}

pub struct SpeechSession {
    adapter: Arc<dyn SpeechAdapter>,
    _guard: tokio::sync::OwnedMutexGuard<()>,
}

impl SpeechSession {
    pub async fn health_check(&self) -> Result<SpeechHealth, SpeechFailure> {
        self.adapter.health_check().await
    }
    pub async fn speak(&self, request: SpeechRequest) -> Result<SpeechResult, SpeechFailure> {
        self.adapter.speak(request).await
    }
    pub async fn control(&self, command: SpeechControl) -> Result<(), SpeechFailure> {
        match command {
            SpeechControl::Pause => self.adapter.pause().await,
            SpeechControl::Resume => self.adapter.resume().await,
            SpeechControl::Skip => self.adapter.skip().await,
            SpeechControl::Clear => self.adapter.clear().await,
        }
    }
}
