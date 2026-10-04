use super::super::model::{is_supported_application_path, validate_paths};
use super::super::ports::ApplicationTargetResolver;
use std::path::{Path, PathBuf};

pub(in crate::launcher) struct FileTargetResolver;
impl ApplicationTargetResolver for FileTargetResolver {
    fn resolve(&self, raw_target: &str) -> Result<PathBuf, String> {
        validate_application_target(raw_target)
    }
}

fn validate_application_target(raw_target: &str) -> Result<PathBuf, String> {
    validate_paths(std::iter::once(raw_target))?;
    let raw_target = raw_target.trim();
    if raw_target.is_empty() {
        return Err("アプリのパスが空です。".to_string());
    }

    let target = Path::new(raw_target);
    if !is_supported_application_path(target) {
        return Err(format!(
            "追加できるのは .exe または .lnk ファイルだけです: {}",
            target.display()
        ));
    }
    if !target.exists() {
        return Err(format!(
            "アプリが見つかりません。移動または削除されていないか確認してください: {}",
            target.display()
        ));
    }
    if !target.is_file() {
        return Err(format!(
            "アプリのファイルを指定してください: {}",
            target.display()
        ));
    }

    target
        .canonicalize()
        .map(normalize_canonical_path)
        .map_err(|error| {
            format!(
                "アプリのパスを確認できませんでした: {} ({error})",
                target.display()
            )
        })
}

#[cfg(target_os = "windows")]
pub(in crate::launcher) fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    // std::fs::canonicalize uses the Win32 verbatim prefix. Explorer,
    // PowerShell's drawing APIs and some applications expect a regular DOS/UNC path.
    let path_text = path.to_string_lossy();
    if let Some(rest) = path_text.strip_prefix("\\\\?\\UNC\\") {
        return PathBuf::from(format!("\\\\{rest}"));
    }
    if let Some(rest) = path_text.strip_prefix("\\\\?\\") {
        return PathBuf::from(rest);
    }
    path
}

#[cfg(not(target_os = "windows"))]
pub(in crate::launcher) fn normalize_canonical_path(path: PathBuf) -> PathBuf {
    path
}
