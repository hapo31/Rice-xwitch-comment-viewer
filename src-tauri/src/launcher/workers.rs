//! Bounded asynchronous orchestration around uncancellable blocking adapters.
use super::model::*;
use super::ports::{
    ApplicationTargetResolver, IconExtractionError, IconExtractionWarning, IconExtractor,
};
use futures_util::{stream, StreamExt, TryStreamExt};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

#[derive(Debug, Clone)]
struct PreparedLauncherItem {
    target: PathBuf,
    icon_data_url: Option<ValidatedLauncherIconDataUrl>,
    icon_warning: Option<IconExtractionWarning>,
}

fn prepare_launcher_item(
    raw_target: String,
    extractor: &dyn IconExtractor,
    resolver: &dyn ApplicationTargetResolver,
) -> Result<PreparedLauncherItem, String> {
    let target = resolver.resolve(&raw_target)?;
    let started_at = Instant::now();
    let (icon_data_url, icon_warning) = match extractor.extract(&target) {
        Ok(icon_data_url) => {
            let validated = icon_data_url
                .map(|icon| ValidatedLauncherIconDataUrl::parse(&icon))
                .transpose()
                .map_err(|_| "アイコンは正しいPNG画像48KiB・128×128以内にしてください。小さいアイコンのアプリを選んでください。追加内容は保存していません。".to_string())?;
            (validated, None)
        },
        Err(IconExtractionError::ResourceLimit) => return Err("アイコン出力は64KiBまでです。小さいアイコンのアプリを選んでください。追加内容は保存していません。".into()),
        Err(IconExtractionError::Failed(message)) => (
            None,
            Some(IconExtractionWarning {
                target: target.clone(),
                message,
                elapsed: started_at.elapsed(),
            }),
        ),
    };

    Ok(PreparedLauncherItem {
        target,
        icon_data_url,
        icon_warning,
    })
}

#[derive(Debug, Clone)]
pub(super) struct BuiltLauncherItems {
    pub items: Vec<LauncherItem>,
    pub icon_warnings: Vec<IconExtractionWarning>,
}

#[derive(Clone)]
pub(super) struct LauncherWorkerConfig {
    pub worker_limit: usize,
    pub worker_pool: Arc<Semaphore>,
    pub acquire_timeout: Duration,
    pub job_timeout: Duration,
}

fn assemble_launcher_items(
    existing: &[LauncherItem],
    prepared: Vec<PreparedLauncherItem>,
) -> Result<BuiltLauncherItems, String> {
    let mut target_keys = existing
        .iter()
        .map(|item| path_identity_key(Path::new(&item.target)))
        .collect::<HashSet<_>>();
    let mut item_ids = existing
        .iter()
        .map(|item| item.id.clone())
        .collect::<HashSet<_>>();
    let mut order = next_order(existing);
    let mut new_items = Vec::with_capacity(prepared.len());
    let mut icon_warnings = Vec::new();

    for prepared in prepared {
        let target = prepared.target;
        if !target_keys.insert(path_identity_key(&target)) {
            continue;
        }
        if existing.len().saturating_add(new_items.len()) >= MAX_LAUNCHER_ITEMS {
            return Err(format!(
                "ランチャーに登録できるアプリは最大 {MAX_LAUNCHER_ITEMS} 件です。"
            ));
        }

        let id = make_item_id(&target, &item_ids);
        item_ids.insert(id.clone());
        new_items.push(LauncherItem {
            id,
            kind: LauncherItemKind::Application,
            target: target.to_string_lossy().into_owned(),
            display_name: derive_display_name(&target),
            icon_data_url: prepared.icon_data_url,
            background_color: None,
            group_id: None,
            order,
        });
        if let Some(warning) = prepared.icon_warning {
            icon_warnings.push(warning);
        }
        order = order.saturating_add(1);
    }

    Ok(BuiltLauncherItems {
        items: new_items,
        icon_warnings,
    })
}

pub(super) async fn build_new_items_in_workers_with_adapters(
    existing: Vec<LauncherItem>,
    raw_targets: Vec<String>,
    extractor: Arc<dyn IconExtractor>,
    resolver: Arc<dyn ApplicationTargetResolver>,
    config: LauncherWorkerConfig,
) -> Result<BuiltLauncherItems, String> {
    validate_paths(raw_targets.iter().map(String::as_str))?;

    if config.worker_limit == 0 {
        return Err("ランチャーのアイコン確認 worker 数が無効です。".to_string());
    }

    let mut raw_keys = HashSet::new();
    let unique_targets = raw_targets
        .into_iter()
        .filter(|target| raw_keys.insert(path_identity_key(Path::new(target.trim()))));
    let prepared = stream::iter(unique_targets.enumerate())
        .map(|(index, raw_target)| {
            let config = config.clone();
            let extractor = Arc::clone(&extractor);
            let resolver = Arc::clone(&resolver);
            async move {
            let permit = tokio::time::timeout(
                config.acquire_timeout,
                config.worker_pool.clone().acquire_owned(),
            )
            .await
            .map_err(|_| {
                "ランチャーのアイコン確認が混み合っています。しばらく待ってからもう一度追加してください。"
                    .to_string()
            })?
            .map_err(|_| "ランチャーのアイコン確認を開始できません。".to_string())?;

            let task = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                prepare_launcher_item(raw_target, extractor.as_ref(), resolver.as_ref())
            });
            let prepared = match tokio::time::timeout(config.job_timeout, task).await {
                Ok(Ok(prepared)) => prepared?,
                Ok(Err(error)) => {
                    return Err(format!("ランチャーのファイル確認に失敗しました: {error}"));
                }
                Err(_) => {
                    // A running blocking task cannot be force-cancelled, so it retains its
                    // semaphore permit until it returns. That cap is the recovery boundary for
                    // hung filesystem calls.
                    return Err(
                        "ランチャーのファイル確認がタイムアウトしました。ネットワーク上のショートカットを確認してください。"
                            .to_string(),
                    );
                }
            };
            Ok::<_, String>((index, prepared))
            }
        })
        .buffer_unordered(config.worker_limit)
        .try_collect::<Vec<_>>()
        .await;

    let mut prepared = prepared?;
    prepared.sort_by_key(|(index, _)| *index);
    assemble_launcher_items(
        &existing,
        prepared.into_iter().map(|(_, item)| item).collect(),
    )
}

#[cfg(test)]
pub(super) async fn build_new_items_in_workers_with_extractor(
    existing: Vec<LauncherItem>,
    raw_targets: Vec<String>,
    extractor: Arc<dyn IconExtractor>,
    config: LauncherWorkerConfig,
) -> Result<BuiltLauncherItems, String> {
    build_new_items_in_workers_with_adapters(
        existing,
        raw_targets,
        extractor,
        Arc::new(super::platform::FileTargetResolver),
        config,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
    use std::sync::Mutex;

    struct ChangingExtractor(Mutex<String>);

    impl IconExtractor for ChangingExtractor {
        fn extract(&self, _: &Path) -> Result<Option<String>, IconExtractionError> {
            Ok(Some(self.0.lock().unwrap().clone()))
        }
    }

    struct Resolver;

    impl ApplicationTargetResolver for Resolver {
        fn resolve(&self, target: &str) -> Result<PathBuf, String> {
            Ok(PathBuf::from(target))
        }
    }

    fn png_data_url() -> (String, Vec<u8>) {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0, 0, 0, 0]).unwrap();
        }
        let url = format!(
            "{LAUNCHER_ICON_DATA_URL_PREFIX}{}",
            BASE64_STANDARD.encode(&bytes)
        );
        (url, bytes)
    }

    #[test]
    fn changed_extracted_icon_bytes_are_validated_again() {
        reset_icon_png_decode_count();
        let (first_url, mut changed_png) = png_data_url();
        let extractor = ChangingExtractor(Mutex::new(first_url));
        let resolver = Resolver;

        let first = prepare_launcher_item("app.exe".into(), &extractor, &resolver)
            .expect("first external icon is valid");
        assert!(first.icon_data_url.is_some());
        assert_eq!(icon_png_decode_count(), 1);

        // Keep a valid PNG shape and base64 encoding while changing its IHDR CRC.
        changed_png[29] ^= 1;
        *extractor.0.lock().unwrap() = format!(
            "{LAUNCHER_ICON_DATA_URL_PREFIX}{}",
            BASE64_STANDARD.encode(changed_png)
        );
        assert!(prepare_launcher_item("app.exe".into(), &extractor, &resolver).is_err());
        assert_eq!(icon_png_decode_count(), 2);
    }
}
