//! Production composition and OS adapters. Service/model never import this layer.
use super::model::LauncherCapabilities;
#[cfg(any(not(feature = "app"), not(target_os = "windows")))]
use super::model::UNSUPPORTED_LAUNCHER_MESSAGE;
use super::ports::{ApplicationLauncher, IconExtractionError, IconExtractor, LaunchContext};
use super::service::LauncherRuntime;
use super::workers::LauncherWorkerConfig;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

#[cfg(any(all(feature = "app", target_os = "windows"), test))]
pub(super) mod process;
#[cfg(any(all(feature = "app", target_os = "windows"), test))]
pub(super) mod shortcut;
mod target;
#[cfg(all(feature = "app", target_os = "windows"))]
mod windows;
#[cfg(test)]
pub(super) use target::normalize_canonical_path;
pub(super) use target::FileTargetResolver;

pub(super) struct SystemIconExtractor;
impl IconExtractor for SystemIconExtractor {
    fn extract(&self, target: &Path) -> Result<Option<String>, IconExtractionError> {
        #[cfg(all(feature = "app", target_os = "windows"))]
        {
            windows::icon::extract_icon_data_url(target)
        }
        #[cfg(any(not(feature = "app"), not(target_os = "windows")))]
        {
            let _ = target;
            Ok(None)
        }
    }
}

struct SystemApplicationLauncher;
impl ApplicationLauncher for SystemApplicationLauncher {
    fn launch(&self, target: &Path, context: &LaunchContext) -> Result<(), String> {
        #[cfg(all(feature = "app", target_os = "windows"))]
        {
            windows::launch::spawn_application(target, context)
        }
        #[cfg(any(not(feature = "app"), not(target_os = "windows")))]
        {
            let _ = (target, context);
            Err(UNSUPPORTED_LAUNCHER_MESSAGE.into())
        }
    }
}

impl Default for LauncherRuntime {
    fn default() -> Self {
        // One runtime is owned by AppState. All commands share its pool. A
        // timed-out blocking task retains a permit until it actually exits.
        const WORKERS: usize = 4;
        Self::new(
            LauncherCapabilities::current(),
            Arc::new(FileTargetResolver),
            Arc::new(SystemIconExtractor),
            Arc::new(SystemApplicationLauncher),
            LauncherWorkerConfig {
                worker_limit: WORKERS,
                worker_pool: Arc::new(Semaphore::new(WORKERS)),
                acquire_timeout: Duration::from_secs(6),
                job_timeout: Duration::from_secs(7),
            },
        )
    }
}
