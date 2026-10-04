use super::super::super::model::{LAUNCHER_ICON_DATA_URL_PREFIX, MAX_ICON_BASE64_LENGTH};
use super::super::super::ports::IconExtractionError;
use super::super::process::{
    read_pipe_bounded, terminate_and_reap_child, wait_for_child_exit, ChildExitWaitError,
};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

const ICON_EXTRACTION_TIMEOUT: Duration = Duration::from_secs(5);

pub(in crate::launcher) fn extract_icon_data_url(
    target: &Path,
) -> Result<Option<String>, IconExtractionError> {
    use std::os::windows::process::CommandExt;

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

    let mut child = Command::new("powershell.exe")
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
        .env("RICE_LAUNCHER_ICON_PATH", target.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("PowerShell を開始できませんでした: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "PowerShell の出力を取得できませんでした。".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "PowerShell のエラー出力を取得できませんでした。".to_string())?;
    let stdout_reader =
        std::thread::spawn(move || read_pipe_bounded(stdout, MAX_ICON_BASE64_LENGTH));
    let stderr_reader = std::thread::spawn(move || read_pipe_bounded(stderr, 8 * 1024));
    let status = match wait_for_child_exit(&mut child, ICON_EXTRACTION_TIMEOUT) {
        Ok(status) => status,
        Err(ChildExitWaitError::TimedOut { termination }) => {
            return match termination {
                Ok(status) => {
                    let stdout_result = stdout_reader.join();
                    let stderr_result = stderr_reader.join();
                    let pipe_error = !matches!(stdout_result, Ok(Ok(_)))
                        || !matches!(stderr_result, Ok(Ok(_)));
                    if pipe_error {
                        Err(format!(
                            "PowerShell のアイコン抽出が {} 秒でタイムアウトしました。子プロセスは終了しました（{status}）が、出力回収を確認できませんでした。",
                            ICON_EXTRACTION_TIMEOUT.as_secs()
                        ))
                    } else {
                        Err(format!(
                            "PowerShell のアイコン抽出が {} 秒でタイムアウトしました。子プロセスの終了を確認しました（{status}）。",
                            ICON_EXTRACTION_TIMEOUT.as_secs()
                        ))
                    }
                }
                Err(error) => Err(format!(
                    "PowerShell のアイコン抽出が {} 秒でタイムアウトしました。子プロセスの終了を確認できませんでした: {error}",
                    ICON_EXTRACTION_TIMEOUT.as_secs()
                )),
            }.map_err(IconExtractionError::from);
        }
        Err(ChildExitWaitError::Wait(error)) => {
            let termination = terminate_and_reap_child(&mut child);
            if termination.is_ok() {
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
            }
            return Err(IconExtractionError::Failed(match termination {
                Ok(status) => format!(
                    "PowerShell の状態を確認できませんでした。子プロセスの終了を確認しました（{status}）: {error}"
                ),
                Err(termination_error) => format!(
                    "PowerShell の状態を確認できませんでした。子プロセスの終了も確認できませんでした: {error}; {termination_error}"
                ),
            }));
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "PowerShell の出力処理が停止しました。".to_string())?
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::InvalidData {
                IconExtractionError::ResourceLimit
            } else {
                IconExtractionError::Failed(format!("PowerShell の出力を読めませんでした: {error}"))
            }
        })?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "PowerShell のエラー出力処理が停止しました。".to_string())?
        .map_err(|error| format!("PowerShell のエラー出力を読めませんでした: {error}"))?;
    if !status.success() {
        let details = String::from_utf8_lossy(&stderr)
            .trim()
            .chars()
            .take(400)
            .collect::<String>();
        return Err(IconExtractionError::Failed(if details.is_empty() {
            format!("PowerShell のアイコン抽出が終了コード {status} で失敗しました。")
        } else {
            format!("PowerShell のアイコン抽出に失敗しました: {details}")
        }));
    }

    let encoded = String::from_utf8(stdout)
        .map_err(|_| "PowerShell のアイコン出力が文字列ではありません。".to_string())?;
    let encoded = encoded.trim();
    if encoded.is_empty() || encoded.len() > MAX_ICON_BASE64_LENGTH {
        return Err(IconExtractionError::ResourceLimit);
    }
    Ok(Some(format!("{LAUNCHER_ICON_DATA_URL_PREFIX}{encoded}")))
}
