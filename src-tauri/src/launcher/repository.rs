//! Atomic bridge to the shared settings transaction. No Tauri dependency.
use super::model::LauncherItem;
use super::ports::SettingsRepository;
use crate::settings::{update_shared_settings_transaction, AppSettings};
use std::sync::Mutex;

pub(super) struct AppSettingsRepository<'a, F> {
    pub settings: &'a Mutex<AppSettings>,
    pub transaction: &'a Mutex<()>,
    pub persist: F,
}

impl<F> SettingsRepository for AppSettingsRepository<'_, F>
where
    F: Fn(&AppSettings) -> Result<(), String> + Send + Sync,
{
    fn snapshot(&self) -> Result<Vec<LauncherItem>, String> {
        launcher_items_snapshot(self.settings)
    }

    fn update(
        &self,
        mutation: &mut dyn FnMut(&mut Vec<LauncherItem>) -> Result<(), String>,
    ) -> Result<Vec<LauncherItem>, String> {
        let (_, settings) = update_shared_settings_transaction(
            self.settings,
            self.transaction,
            |candidate| mutation(&mut candidate.launcher.items),
            |candidate| (self.persist)(candidate),
        )?;
        Ok(settings.launcher.items)
    }
}

pub(super) fn launcher_items_snapshot(
    settings: &Mutex<AppSettings>,
) -> Result<Vec<LauncherItem>, String> {
    settings
        .lock()
        .map_err(|error| error.to_string())
        .map(|settings| settings.launcher.items.clone())
}
