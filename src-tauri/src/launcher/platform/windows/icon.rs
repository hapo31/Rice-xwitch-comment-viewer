use super::super::super::model::{LAUNCHER_ICON_DATA_URL_PREFIX, MAX_ICON_BASE64_LENGTH};
use super::super::super::ports::IconExtractionError;
use super::super::process::{run_bounded, ProcessRunError};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

const ICON_EXTRACTION_TIMEOUT: Duration = Duration::from_secs(5);

pub(in crate::launcher) fn extract_icon_data_url(
    target: &Path,
) -> Result<Option<String>, IconExtractionError> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const EXTRACT_ICON_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$path = $env:RICE_LAUNCHER_ICON_PATH
$source = $path
if ([IO.Path]::GetExtension($path) -ieq '.lnk') {
  $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($path)
  $iconLocation = ($shortcut.IconLocation -split ',')[0].Trim('"')
  if ($iconLocation -and [IO.File]::Exists($iconLocation)) {
    $source = $iconLocation
  } elseif ($shortcut.TargetPath -and [IO.File]::Exists($shortcut.TargetPath)) {
    $source = $shortcut.TargetPath
  }
}
$icon = [Drawing.Icon]::ExtractAssociatedIcon($source)
if ($null -eq $icon) { exit 2 }
$bitmap = $icon.ToBitmap()
$stream = New-Object IO.MemoryStream
try {
  $bitmap.Save($stream, [Drawing.Imaging.ImageFormat]::Png)
  [Console]::Out.Write([Convert]::ToBase64String($stream.ToArray()))
} finally {
  $stream.Dispose()
  $bitmap.Dispose()
  $icon.Dispose()
}
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
            EXTRACT_ICON_SCRIPT,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .env("RICE_LAUNCHER_ICON_PATH", target.as_os_str());

    // IconExtractor remains synchronous because Launcher filesystem and COM
    // work share one blocking worker/permit. Tokio documents Handle::block_on
    // for this bridge from a spawn_blocking thread; process I/O stays on the
    // app runtime instead of creating reader threads here.
    let output = tokio::runtime::Handle::current()
        .block_on(run_bounded(
            command,
            ICON_EXTRACTION_TIMEOUT,
            MAX_ICON_BASE64_LENGTH,
            8 * 1024,
        ))
        .map_err(|error| match error {
            ProcessRunError::OutputLimit { stdout: true, .. } => IconExtractionError::ResourceLimit,
            other => IconExtractionError::Failed(
                other.message("PowerShell のアイコン抽出", ICON_EXTRACTION_TIMEOUT),
            ),
        })?;

    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr)
            .trim()
            .chars()
            .take(400)
            .collect::<String>();
        return Err(IconExtractionError::Failed(if details.is_empty() {
            format!(
                "PowerShell のアイコン抽出が終了コード {} で失敗しました。",
                output.status
            )
        } else {
            format!("PowerShell のアイコン抽出に失敗しました: {details}")
        }));
    }

    let encoded = String::from_utf8(output.stdout)
        .map_err(|_| "PowerShell のアイコン出力が文字列ではありません。".to_string())?;
    let encoded = encoded.trim();
    if encoded.is_empty() || encoded.len() > MAX_ICON_BASE64_LENGTH {
        return Err(IconExtractionError::ResourceLimit);
    }
    Ok(Some(format!("{LAUNCHER_ICON_DATA_URL_PREFIX}{encoded}")))
}
