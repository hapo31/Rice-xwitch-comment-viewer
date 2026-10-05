use std::io::Read;
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

/// Capture only backend-generated commands. Both pipes are drained with limits,
/// and a timeout kills/reaps our child before reporting confirmed termination.
#[cfg(all(test, feature = "app", target_os = "windows"))]
pub(in crate::launcher) fn capture_bounded(
    mut command: std::process::Command,
    timeout: Duration,
    stdout_limit: usize,
) -> Result<Vec<u8>, String> {
    use std::process::Stdio;
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("ショートカット確認を開始できませんでした: {error}"))?;
    // A closed pipe is an explicit EOF, not Windows' NUL device. Never leave
    // an input writer open in a noninteractive helper (including PowerShell).
    drop(child.stdin.take());
    let stdout = child.stdout.take().expect("configured stdout pipe");
    let stderr = child.stderr.take().expect("configured stderr pipe");
    let stdout_reader = std::thread::spawn(move || read_pipe_bounded(stdout, stdout_limit));
    let stderr_reader = std::thread::spawn(move || read_pipe_bounded(stderr, 8 * 1024));
    let status = match wait_for_child_exit(&mut child, timeout) {
        Ok(status) => status,
        Err(ChildExitWaitError::TimedOut { termination }) => {
            return match termination {
                Ok(_) => {
                    let output = stdout_reader.join();
                    let errors = stderr_reader.join();
                    if matches!(output, Ok(Ok(_))) && matches!(errors, Ok(Ok(_))) {
                        let details = match &errors {
                            Ok(Ok(bytes)) => String::from_utf8_lossy(bytes).chars().take(400).collect::<String>(),
                            _ => String::new(),
                        };
                        Err(format!("ショートカット確認がタイムアウトしました。確認用processの終了・出力回収を確認しました。{details}"))
                    } else {
                        Err("ショートカット確認がタイムアウトしました。確認用processは終了しましたが、出力回収を確認できませんでした。".into())
                    }
                }
                Err(error) => Err(format!("ショートカット確認がタイムアウトしました。確認用processの終了を確認できませんでした: {error}")),
            };
        }
        Err(ChildExitWaitError::Wait(error)) => {
            let terminated = terminate_and_reap_child(&mut child);
            if terminated.is_ok() {
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
            }
            return Err(format!("ショートカット確認processの状態を確認できませんでした: {error}; 終了処理={terminated:?}"));
        }
    };
    // Join both before propagating either error (an oversized stdout must not
    // leave an unjoined stderr reader behind).
    let output = stdout_reader.join();
    let errors = stderr_reader.join();
    let output = output
        .map_err(|_| "ショートカットの出力処理が停止しました。".to_string())?
        .map_err(|error| {
            format!("ショートカットの出力を取得できませんでした（容量上限を含む）: {error}")
        })?;
    let errors = errors
        .map_err(|_| "ショートカットのエラー処理が停止しました。".to_string())?
        .map_err(|error| format!("ショートカットのエラー出力を取得できませんでした: {error}"))?;
    if !status.success() {
        let details: String = String::from_utf8_lossy(&errors).chars().take(400).collect();
        return Err(format!(
            "ショートカットを解決できませんでした: {status} {details}"
        ));
    }
    Ok(output)
}

pub(in crate::launcher) enum ChildExitWaitError {
    Wait(std::io::Error),
    TimedOut {
        termination: Result<ExitStatus, String>,
    },
}

pub(in crate::launcher) fn wait_for_child_exit(
    child: &mut Child,
    timeout: Duration,
) -> Result<ExitStatus, ChildExitWaitError> {
    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started_at.elapsed() >= timeout => {
                return Err(ChildExitWaitError::TimedOut {
                    termination: terminate_and_reap_child(child),
                });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => return Err(ChildExitWaitError::Wait(error)),
        }
    }
}

pub(in crate::launcher) fn terminate_and_reap_child(
    child: &mut Child,
) -> Result<ExitStatus, String> {
    match child.kill() {
        Ok(()) => child
            .wait()
            .map_err(|error| format!("終了を待機できませんでした: {error}")),
        Err(kill_error) => match child.try_wait() {
            Ok(Some(status)) => Ok(status),
            Ok(None) => Err(format!("終了要求を送れませんでした: {kill_error}")),
            Err(wait_error) => Err(format!(
                "終了要求を送れず、状態も確認できませんでした: {kill_error}; {wait_error}"
            )),
        },
    }
}

pub(in crate::launcher) fn read_pipe_bounded(
    mut pipe: impl Read,
    maximum: usize,
) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::with_capacity(maximum.min(8 * 1024));
    let mut buffer = [0_u8; 4096];
    let mut exceeded = false;
    loop {
        let read = pipe.read(&mut buffer)?;
        if read == 0 {
            if exceeded {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "icon extraction output exceeds limit",
                ));
            }
            return Ok(output);
        }
        let remaining = maximum.saturating_sub(output.len());
        exceeded |= read > remaining;
        output.extend_from_slice(&buffer[..read.min(remaining)]);
    }
}
