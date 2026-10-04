//! Registration/launch use cases: no Tauri, concrete persistence or OS APIs.
use super::model::*;
use super::ports::{
    ApplicationLauncher, ApplicationTargetResolver, IconExtractor, LauncherEventSink,
    SettingsRepository,
};
use super::workers::{
    build_new_items_in_workers_with_adapters, BuiltLauncherItems, LauncherWorkerConfig,
};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct LauncherRuntime {
    capabilities: LauncherCapabilities,
    resolver: Arc<dyn ApplicationTargetResolver>,
    extractor: Arc<dyn IconExtractor>,
    launcher: Arc<dyn ApplicationLauncher>,
    workers: LauncherWorkerConfig,
}
impl LauncherRuntime {
    pub(super) fn new(
        capabilities: LauncherCapabilities,
        resolver: Arc<dyn ApplicationTargetResolver>,
        extractor: Arc<dyn IconExtractor>,
        launcher: Arc<dyn ApplicationLauncher>,
        workers: LauncherWorkerConfig,
    ) -> Self {
        Self {
            capabilities,
            resolver,
            extractor,
            launcher,
            workers,
        }
    }

    pub(super) fn service<'a>(
        &'a self,
        repository: &'a dyn SettingsRepository,
        events: &'a dyn LauncherEventSink,
    ) -> LauncherService<'a> {
        LauncherService {
            runtime: self,
            repository,
            events,
        }
    }

    fn launch_items(&self, items: &[LauncherItem]) -> LauncherLaunchResult {
        let mut result = LauncherLaunchResult::default();
        for item in items {
            // Kind dispatch belongs here. An Application adapter never sees a
            // Website and cannot accidentally open it via an executable shell.
            let launched = self.capabilities.ensure_supported().and_then(|()| match item.kind {
                LauncherItemKind::Application => self.resolver.resolve(&item.target).and_then(|target| {
                    self.launcher.launch(&target).map_err(|error| format!(
                        "アプリを起動できませんでした。ファイルの場所や実行権限を確認してください: {error}"
                    ))
                }),
                LauncherItemKind::Website => Err("この種類のランチャー項目はまだ起動できません。".into()),
            });
            match launched {
                Ok(()) => result.launched_count += 1,
                Err(message) => result.failures.push(LauncherLaunchFailure {
                    item_id: item.id.clone(),
                    display_name: item.display_name.clone(),
                    message,
                }),
            }
        }
        result
    }
}

pub(super) struct LauncherService<'a> {
    runtime: &'a LauncherRuntime,
    repository: &'a dyn SettingsRepository,
    events: &'a dyn LauncherEventSink,
}
impl LauncherService<'_> {
    pub async fn add(&self, paths: Vec<String>) -> Result<Vec<LauncherItem>, String> {
        self.runtime.capabilities.ensure_supported()?;
        // Snapshot only. Never hold the repository mutex across FS/COM work.
        let existing = self.repository.snapshot()?;
        let BuiltLauncherItems {
            items: built_items,
            icon_warnings,
        } = build_new_items_in_workers_with_adapters(
            existing,
            paths,
            self.runtime.extractor.clone(),
            self.runtime.resolver.clone(),
            self.runtime.workers.clone(),
        )
        .await?;
        let mut built_items = Some(built_items);
        let mut added_count = 0;
        let items = self.repository.update(&mut |current| {
            let built_items = built_items
                .take()
                .ok_or_else(|| "ランチャーの保存処理を繰り返すことはできません。".to_string())?;
            let new_items = merge_new_launcher_items(current, built_items)?;
            added_count = new_items.len();
            current.extend(new_items);
            Ok(())
        })?;
        self.events.added(added_count);
        self.events.icon_warnings(&icon_warnings);
        Ok(items)
    }

    pub fn remove(&self, item_id: &str) -> Result<Vec<LauncherItem>, String> {
        let item_id = item_id.trim();
        let mut removed = None;
        let items = self.repository.update(&mut |current| {
            let index = current
                .iter()
                .position(|item| item.id == item_id)
                .ok_or_else(|| "削除するランチャー項目が見つかりません。".to_string())?;
            removed = Some(current.remove(index));
            Ok(())
        })?;
        if let Some(removed) = removed {
            self.events.removed(&removed);
        }
        Ok(items)
    }

    pub fn launch(&self, item_id: &str) -> LauncherLaunchResult {
        let result = match self.repository.snapshot() {
            Ok(items) => match items.into_iter().find(|item| item.id == item_id.trim()) {
                Some(item) => self.runtime.launch_items(&[item]),
                None => load_failure(
                    item_id,
                    "アプリ",
                    "起動するランチャー項目が見つかりません。".into(),
                ),
            },
            Err(error) => load_failure(
                item_id,
                "アプリ",
                format!("ランチャーの設定を読み込めませんでした: {error}"),
            ),
        };
        self.events.launched(&result);
        result
    }

    pub fn launch_all(&self) -> LauncherLaunchResult {
        let result = match self.repository.snapshot() {
            Ok(mut items) => {
                items.sort_by_key(|item| item.order);
                self.runtime.launch_items(&items)
            }
            Err(error) => load_failure(
                "",
                "ランチャー",
                format!("ランチャーの設定を読み込めませんでした: {error}"),
            ),
        };
        self.events.launched(&result);
        result
    }
}

fn load_failure(item_id: &str, display_name: &str, message: String) -> LauncherLaunchResult {
    LauncherLaunchResult {
        launched_count: 0,
        failures: vec![LauncherLaunchFailure {
            item_id: item_id.into(),
            display_name: display_name.into(),
            message,
        }],
    }
}

pub(super) fn merge_new_launcher_items(
    existing: &[LauncherItem],
    new_items: Vec<LauncherItem>,
) -> Result<Vec<LauncherItem>, String> {
    let mut target_keys = existing
        .iter()
        .map(|item| path_identity_key(Path::new(&item.target)))
        .collect::<HashSet<_>>();
    let mut item_ids = existing
        .iter()
        .map(|item| item.id.clone())
        .collect::<HashSet<_>>();
    let mut order = next_order(existing);
    let mut merged = Vec::new();

    for mut item in new_items {
        let target = Path::new(&item.target);
        if !target_keys.insert(path_identity_key(target)) {
            continue;
        }
        if existing.len().saturating_add(merged.len()) >= MAX_LAUNCHER_ITEMS {
            return Err(item_limit_message());
        }
        item.id = make_item_id(target, &item_ids);
        item_ids.insert(item.id.clone());
        item.order = order;
        order = order.saturating_add(1);
        merged.push(item);
    }
    // Validate the entire candidate, not just the additions, before any save.
    let mut candidate = existing.to_vec();
    candidate.extend(merged.iter().cloned());
    validate_launcher_resources(&candidate)?;
    Ok(merged)
}

#[cfg(all(test, feature = "app", not(target_os = "windows")))]
pub(super) fn launch_items(items: &[LauncherItem]) -> LauncherLaunchResult {
    LauncherRuntime::default().launch_items(items)
}

#[cfg(test)]
mod tests;
