use super::{
    SpeechAdapter, SpeechControl, SpeechFailure, SpeechFuture, SpeechHealth,
    SpeechPlaybackCompletion, SpeechRequest, SpeechResult,
};
use crate::settings::{AppState, SpeechSettings};
use std::sync::{Arc, Mutex};
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
    active: ActivePlaybackRegistry,
    destination_policy: Arc<super::destination::DestinationPolicy>,
    factory: Arc<dyn SpeechAdapterFactory>,
    pub(crate) clock: Arc<dyn SpeechClock>,
}

impl Default for SpeechRuntime {
    fn default() -> Self {
        let policy = Arc::new(super::destination::DestinationPolicy::default());
        let mut runtime = Self::new(
            Arc::new(super::factory::ConfiguredAdapterFactory {
                policy: policy.clone(),
            }),
            Arc::new(SystemSpeechClock),
        );
        runtime.destination_policy = policy;
        runtime
    }
}

impl SpeechRuntime {
    pub fn new(factory: Arc<dyn SpeechAdapterFactory>, clock: Arc<dyn SpeechClock>) -> Self {
        Self {
            dispatcher: SpeechDispatcher::default(),
            active: Arc::default(),
            destination_policy: Arc::default(),
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
            active: self.active.clone(),
            confirmation_text: self.factory.connection_confirmation(settings),
        })
    }

    pub(crate) fn dispatcher(&self) -> SpeechDispatcher {
        self.dispatcher.clone()
    }

    /// Choose after acquiring the gate: a talk already holding it may establish
    /// playback while this control is waiting. Configuration is only a fallback.
    pub async fn lock_control(
        &self,
        configured: impl FnOnce() -> Result<SelectedSpeechAdapter, SpeechFailure>,
    ) -> Result<SpeechSession, SpeechFailure> {
        let guard = self.dispatcher.clone().lock_owned().await;
        let active = self
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let adapter = match active {
            Some(playback) => playback.adapter.clone(),
            None => configured()?.adapter,
        };
        Ok(SpeechSession {
            adapter,
            active: self.active.clone(),
            _guard: guard,
        })
    }
    pub(crate) fn destination_policy(&self) -> Arc<super::destination::DestinationPolicy> {
        self.destination_policy.clone()
    }
}

/// Keeps raw adapter calls behind a common ordering gate. Holding this session
/// spans request reservation OR remote control + its local application/events.
#[derive(Clone)]
pub struct SelectedSpeechAdapter {
    adapter: Arc<dyn SpeechAdapter>,
    dispatcher: SpeechDispatcher,
    active: ActivePlaybackRegistry,
    pub(crate) confirmation_text: Option<String>,
}

impl SelectedSpeechAdapter {
    pub async fn lock(&self) -> SpeechSession {
        SpeechSession {
            adapter: self.adapter.clone(),
            active: self.active.clone(),
            _guard: self.dispatcher.clone().lock_owned().await,
        }
    }
    pub(crate) fn session_after_dispatch_lock(
        &self,
        guard: tokio::sync::OwnedMutexGuard<()>,
    ) -> SpeechSession {
        SpeechSession {
            adapter: self.adapter.clone(),
            active: self.active.clone(),
            _guard: guard,
        }
    }
}

pub struct SpeechSession {
    adapter: Arc<dyn SpeechAdapter>,
    active: ActivePlaybackRegistry,
    _guard: tokio::sync::OwnedMutexGuard<()>,
}

type ActivePlaybackRegistry = Arc<Mutex<Option<Arc<PlaybackIdentity>>>>;

struct PlaybackIdentity {
    adapter: Arc<dyn SpeechAdapter>,
    _request_id: String,
}

/// Owns the exact adapter from acceptance until local completion. Pointer identity
/// is a session generation, so dropping an old owner cannot clear a newer one.
pub(crate) struct ActiveSpeechPlayback {
    identity: Arc<PlaybackIdentity>,
    active: ActivePlaybackRegistry,
}

impl ActiveSpeechPlayback {
    pub async fn wait_for_completion(&self) -> SpeechPlaybackCompletion {
        self.identity.adapter.wait_for_completion().await
    }
}

impl Drop for ActiveSpeechPlayback {
    fn drop(&mut self) {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active
            .as_ref()
            .is_some_and(|entry| Arc::ptr_eq(entry, &self.identity))
        {
            *active = None;
        }
    }
}

impl SpeechSession {
    pub async fn health_check(&self) -> Result<SpeechHealth, SpeechFailure> {
        self.adapter.health_check().await
    }
    pub async fn speak(&self, request: SpeechRequest) -> Result<SpeechResult, SpeechFailure> {
        self.adapter.speak(request).await
    }
    pub(crate) async fn speak_for_queue(
        &self,
        request: SpeechRequest,
    ) -> Result<ActiveSpeechPlayback, SpeechFailure> {
        let request_id = request.id.clone();
        self.adapter.speak(request).await?;
        let identity = Arc::new(PlaybackIdentity {
            adapter: self.adapter.clone(),
            _request_id: request_id,
        });
        *self.active.lock().unwrap_or_else(|e| e.into_inner()) = Some(identity.clone());
        Ok(ActiveSpeechPlayback {
            identity,
            active: self.active.clone(),
        })
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
