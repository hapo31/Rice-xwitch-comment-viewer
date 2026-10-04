//! Small, object-safe boundaries. No Tauri, filesystem or process implementation.
use super::model::{LauncherItem, LauncherLaunchResult};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(super) trait ApplicationTargetResolver: Send + Sync {
    fn resolve(&self, raw_target: &str) -> Result<PathBuf, String>;
}

pub(super) trait IconExtractor: Send + Sync {
    fn extract(&self, target: &Path) -> Result<Option<String>, IconExtractionError>;
}

#[derive(Debug, Clone)]
pub(super) enum IconExtractionError {
    Failed(String),
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    ResourceLimit,
}
impl From<String> for IconExtractionError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

/// Receives only an application path, never a LauncherItem or its kind.
/// Adding Website dispatch cannot add a match arm to this adapter.
pub(super) trait ApplicationLauncher: Send + Sync {
    /// Success means a target process was created, not that the app is ready.
    fn launch(&self, target: &Path, context: &LaunchContext) -> Result<(), String>;
}

/// Blocking work retains its permit after timeout. A late resolver must not
/// then start an application. Check immediately before the OS process call.
#[derive(Clone)]
pub(super) struct LaunchContext {
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}
impl LaunchContext {
    pub fn new(timeout: Duration) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + timeout,
        }
    }
    pub fn ensure_active(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::SeqCst) || Instant::now() >= self.deadline {
            Err("起動確認の期限を過ぎたため、後からの起動処理を中止しました。".into())
        } else {
            Ok(())
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

pub(super) trait SettingsRepository: Send + Sync {
    fn snapshot(&self) -> Result<Vec<LauncherItem>, String>;
    /// Apply exactly once to a candidate based on the latest state. Validate and
    /// persist before publication; errors must leave the shared state unchanged.
    /// The callback must not perform OS work or call back into this repository.
    fn update(
        &self,
        mutation: &mut dyn FnMut(&mut Vec<LauncherItem>) -> Result<(), String>,
    ) -> Result<Vec<LauncherItem>, String>;
}

#[derive(Debug, Clone)]
pub(super) struct IconExtractionWarning {
    pub target: PathBuf,
    pub message: String,
    pub elapsed: Duration,
}

pub(super) trait LauncherEventSink: Send + Sync {
    fn added(&self, count: usize);
    fn removed(&self, item: &LauncherItem);
    fn launched(&self, result: &LauncherLaunchResult);
    fn icon_warnings(&self, warnings: &[IconExtractionWarning]);
}

#[cfg(test)]
pub(super) use IconExtractor as LauncherIconExtractor;
