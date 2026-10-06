//! Stable public model facade; implementations are deliberately layer-local.
#[cfg(test)]
pub(crate) use model::{LauncherAddResult, LauncherLaunchResult};
#[cfg(feature = "app")]
pub(crate) mod commands;
#[cfg(feature = "app")]
mod events;
mod model;
mod platform;
mod ports;
mod repository;
mod service;
mod workers;

#[cfg(test)]
pub(crate) use model::normalize_launcher_items;
pub(crate) use model::{
    apply_launcher_edits, normalize_launcher_icon_data_url, preflight_launcher_patch,
    validate_launcher_resources, validate_launcher_structure, LauncherCapabilities, LauncherItem,
    LauncherItemKind, LauncherSettings, LauncherSettingsPatch,
};
#[cfg(feature = "app")]
pub(crate) use service::LauncherRuntime;

#[cfg(test)]
use crate::settings::AppSettings;
#[cfg(test)]
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
#[cfg(test)]
use model::*;
#[cfg(test)]
use platform::normalize_canonical_path;
#[cfg(test)]
use platform::process::{read_pipe_bounded, wait_for_child_exit, ChildExitWaitError};
#[cfg(test)]
use ports::*;
#[cfg(test)]
use repository::launcher_items_snapshot;
#[cfg(test)]
use service::*;
#[cfg(test)]
use std::{io::Cursor, path::Path, sync::Arc, time::Duration};
#[cfg(test)]
use tokio::sync::Semaphore;
#[cfg(test)]
use workers::*;

#[cfg(test)]
pub(crate) mod bounds_tests;
#[cfg(test)]
mod tests;
