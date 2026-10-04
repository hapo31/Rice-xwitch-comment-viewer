use super::super::super::ports::ApplicationTargetResolver;
use super::super::process::capture_bounded;
use super::super::shortcut::{
    validate_header, ShortcutMetadata, MAX_SHORTCUT_JSON_BYTES, SHORTCUT_REPAIR,
};
use super::super::target::FileTargetResolver;
use std::fs::File;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub(super) struct ResolvedShortcut {
    pub executable: PathBuf,
    pub working_directory: PathBuf,
    pub metadata: ShortcutMetadata,
}

/// Read on every launch; never persist a resolved target or trigger link tracking
/// / installer repair / another-user activation / a link-repair dialog.
pub(super) fn resolve(shortcut_path: &Path) -> Result<ResolvedShortcut, String> {
    let file = File::open(shortcut_path)
        .map_err(|error| format!("ショートカットを開けませんでした: {error}。{SHORTCUT_REPAIR}"))?;
    let file_bytes = file
        .metadata()
        .map_err(|error| format!("ショートカットの大きさを確認できませんでした: {error}"))?
        .len();
    let mut header = [0u8; 76];
    // Reject the file size before even reading the fixed header/asking COM.
    if file_bytes > super::super::shortcut::MAX_SHORTCUT_FILE_BYTES {
        return Err(
            "ショートカットは1MiB以内にしてください。正しいアプリを再登録してください。".into(),
        );
    }
    file.take(76)
        .read_exact(&mut header)
        .map_err(|_| format!("ショートカットの形式が壊れています。{SHORTCUT_REPAIR}"))?;
    validate_header(&header, file_bytes)?;
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)
$link = (New-Object -ComObject WScript.Shell).CreateShortcut($env:RICE_SHORTCUT_PATH)
$icon = [string]$link.IconLocation
$comma = $icon.LastIndexOf(',')
if ($comma -ge 0) {
  $index = 0
  if ([int]::TryParse($icon.Substring($comma + 1).Trim(), [ref]$index)) { $icon = $icon.Substring(0, $comma) }
}
$details = @{
  target = [Environment]::ExpandEnvironmentVariables([string]$link.TargetPath)
  arguments = [string]$link.Arguments
  workingDirectory = [Environment]::ExpandEnvironmentVariables([string]$link.WorkingDirectory)
  iconSource = [Environment]::ExpandEnvironmentVariables($icon.Trim('"'))
}
[Console]::Out.Write(($details | ConvertTo-Json -Compress))
"#;
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            SCRIPT,
        ])
        .creation_flags(0x0800_0000)
        .env("RICE_SHORTCUT_PATH", shortcut_path)
        .env_remove("PSModulePath");
    let bytes = capture_bounded(command, Duration::from_secs(5), MAX_SHORTCUT_JSON_BYTES)
        .map_err(|error| format!("{error} {SHORTCUT_REPAIR}"))?;
    let metadata = ShortcutMetadata::decode(&bytes)?;
    let executable = FileTargetResolver.resolve(&metadata.target).map_err(|_| {
        format!(
            "ショートカットのリンク先が存在しないか実行ファイルではありません。{SHORTCUT_REPAIR}"
        )
    })?;
    let working_directory = if metadata.working_directory.is_empty() {
        executable
            .parent()
            .ok_or_else(|| format!("リンク先の作業フォルダーを確認できません。{SHORTCUT_REPAIR}"))?
            .to_path_buf()
    } else {
        let path = PathBuf::from(&metadata.working_directory);
        if !path.is_dir() {
            return Err(format!(
                "ショートカットの作業フォルダーが見つかりません。{SHORTCUT_REPAIR}"
            ));
        }
        path.canonicalize()
            .map(super::super::target::normalize_canonical_path)
            .map_err(|_| {
                format!("ショートカットの作業フォルダーを確認できません。{SHORTCUT_REPAIR}")
            })?
    };
    Ok(ResolvedShortcut {
        executable,
        working_directory,
        metadata,
    })
}
