//! Bounded Tokio process execution for Launcher-owned helper processes.
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;

const TERMINATED_PIPE_DRAIN_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub(in crate::launcher) struct BoundedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub(in crate::launcher) enum ProcessRunError {
    Spawn(io::Error),
    Wait {
        error: io::Error,
        termination: Result<ExitStatus, String>,
    },
    TimedOut {
        termination: Result<ExitStatus, String>,
        output_drained: bool,
    },
    Cancelled {
        termination: Result<ExitStatus, String>,
    },
    OutputRead(io::Error),
    OutputTask(String),
    OutputLimit {
        stdout: bool,
        stderr: bool,
    },
}

impl ProcessRunError {
    pub(in crate::launcher) fn message(&self, process_name: &str, timeout: Duration) -> String {
        match self {
            Self::Spawn(error) => format!("{process_name} を開始できませんでした: {error}"),
            Self::Wait { error, termination } => match termination {
                Ok(status) => format!(
                    "{process_name} の状態を確認できませんでした。終了を確認しました（{status}）: {error}"
                ),
                Err(termination_error) => format!(
                    "{process_name} の状態を確認できず、終了も確認できませんでした: {error}; {termination_error}"
                ),
            },
            Self::TimedOut {
                termination,
                output_drained,
            } => match termination {
                Ok(status) if *output_drained => format!(
                    "{process_name} が {} 秒でタイムアウトしました。子プロセスを終了して出力を回収しました（{status}）。",
                    timeout.as_secs()
                ),
                Ok(status) => format!(
                    "{process_name} が {} 秒でタイムアウトしました。子プロセスは終了しました（{status}）が、出力回収を確認できませんでした。",
                    timeout.as_secs()
                ),
                Err(error) => format!(
                    "{process_name} が {} 秒でタイムアウトしました。子プロセスの終了を確認できませんでした: {error}",
                    timeout.as_secs()
                ),
            },
            Self::Cancelled { termination } => match termination {
                Ok(status) => format!("{process_name} をキャンセルし、子プロセスを終了しました（{status}）。"),
                Err(error) => format!("{process_name} をキャンセルしましたが、子プロセスの終了を確認できませんでした: {error}"),
            },
            Self::OutputRead(error) => format!("{process_name} の出力を取得できませんでした: {error}"),
            Self::OutputTask(error) => format!("{process_name} の出力処理が停止しました: {error}"),
            Self::OutputLimit { stdout, stderr } => {
                let streams = match (*stdout, *stderr) {
                    (true, true) => "標準出力と標準エラー出力",
                    (true, false) => "標準出力",
                    (false, true) => "標準エラー出力",
                    (false, false) => "出力",
                };
                format!("{process_name} の{streams}が容量上限を超えました。")
            }
        }
    }
}

#[derive(Debug)]
struct LimitedBytes {
    bytes: Vec<u8>,
    exceeded: bool,
}

/// Run a helper without collecting unbounded output. Both pipes keep draining
/// after their individual storage limit is reached, so a full pipe cannot
/// deadlock the child while it is being waited on.
pub(in crate::launcher) async fn run_bounded(
    command: Command,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedOutput, ProcessRunError> {
    let terminate: Terminator = Arc::new(|child| Box::pin(terminate_and_reap(child)));
    run_bounded_with_terminator(command, timeout, stdout_limit, stderr_limit, terminate).await
}

type TerminationFuture<'a> = Pin<Box<dyn Future<Output = Result<ExitStatus, String>> + Send + 'a>>;
type Terminator = Arc<dyn for<'a> Fn(&'a mut Child) -> TerminationFuture<'a> + Send + Sync>;

async fn run_bounded_with_terminator(
    command: Command,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    terminate: Terminator,
) -> Result<BoundedOutput, ProcessRunError> {
    let (cancel, cancelled) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(run_child(
        command,
        timeout,
        stdout_limit,
        stderr_limit,
        cancelled,
        terminate,
    ));
    let mut cancel_on_drop = CancelOnDrop(Some(cancel));
    let result = task
        .await
        .map_err(|error| ProcessRunError::OutputTask(error.to_string()))?;
    cancel_on_drop.0 = None;
    result
}

struct CancelOnDrop(Option<tokio::sync::oneshot::Sender<()>>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(cancel) = self.0.take() {
            let _ = cancel.send(());
        }
    }
}

async fn run_child(
    mut command: Command,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    mut cancelled: tokio::sync::oneshot::Receiver<()>,
    terminate: Terminator,
) -> Result<BoundedOutput, ProcessRunError> {
    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(ProcessRunError::Spawn)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ProcessRunError::OutputTask("stdout pipe がありません".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ProcessRunError::OutputTask("stderr pipe がありません".into()))?;
    let mut readers = ReaderTasks {
        stdout: ReaderState::Pending(tokio::spawn(read_bounded(stdout, stdout_limit))),
        stderr: ReaderState::Pending(tokio::spawn(read_bounded(stderr, stderr_limit))),
    };

    let deadline = tokio::time::Instant::now() + timeout;
    enum Completed {
        Cancelled,
        Timeout,
        Finished(Result<(ExitStatus, LimitedBytes, LimitedBytes), CompletionError>),
    }
    let completed = {
        let future = async {
            let status = child.wait().await.map_err(CompletionError::Wait)?;
            let (stdout, stderr) = readers.join().await.map_err(CompletionError::Output)?;
            Ok::<_, CompletionError>((status, stdout, stderr))
        };
        tokio::pin!(future);
        tokio::select! {
            biased;
            _ = &mut cancelled => Completed::Cancelled,
            result = tokio::time::timeout_at(deadline, &mut future) => match result {
                Ok(result) => Completed::Finished(result),
                Err(_) => Completed::Timeout,
            }
        }
    };
    match completed {
        Completed::Cancelled => {
            let termination = terminate(&mut child).await;
            let _ = finish_readers_after_termination(&mut readers).await;
            Err(ProcessRunError::Cancelled { termination })
        }
        Completed::Timeout => {
            let termination = terminate(&mut child).await;
            let output_drained = finish_readers_after_termination(&mut readers).await;
            Err(ProcessRunError::TimedOut {
                termination,
                output_drained,
            })
        }
        Completed::Finished(Err(CompletionError::Wait(error))) => {
            let termination = terminate(&mut child).await;
            let _ = finish_readers_after_termination(&mut readers).await;
            Err(ProcessRunError::Wait { error, termination })
        }
        Completed::Finished(Err(CompletionError::Output(error))) => Err(error),
        Completed::Finished(Ok((status, stdout, stderr))) => {
            if stdout.exceeded || stderr.exceeded {
                Err(ProcessRunError::OutputLimit {
                    stdout: stdout.exceeded,
                    stderr: stderr.exceeded,
                })
            } else {
                Ok(BoundedOutput {
                    status,
                    stdout: stdout.bytes,
                    stderr: stderr.bytes,
                })
            }
        }
    }
}

enum CompletionError {
    Wait(io::Error),
    Output(ProcessRunError),
}

enum ReaderState {
    Pending(JoinHandle<io::Result<LimitedBytes>>),
    Complete(Result<io::Result<LimitedBytes>, tokio::task::JoinError>),
    Taken,
}

struct ReaderTasks {
    stdout: ReaderState,
    stderr: ReaderState,
}

impl ReaderTasks {
    async fn join(&mut self) -> Result<(LimitedBytes, LimitedBytes), ProcessRunError> {
        std::future::poll_fn(|context| {
            poll_reader(&mut self.stdout, context);
            poll_reader(&mut self.stderr, context);
            if matches!(&self.stdout, ReaderState::Complete(_))
                && matches!(&self.stderr, ReaderState::Complete(_))
            {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;

        let stdout = take_reader_result(&mut self.stdout);
        let stderr = take_reader_result(&mut self.stderr);
        Ok((stdout?, stderr?))
    }

    fn abort_pending(&mut self) {
        for state in [&mut self.stdout, &mut self.stderr] {
            if let ReaderState::Pending(handle) = state {
                handle.abort();
            }
        }
    }
}

fn poll_reader(state: &mut ReaderState, context: &mut Context<'_>) {
    let result = match state {
        ReaderState::Pending(handle) => Pin::new(handle).poll(context),
        ReaderState::Complete(_) | ReaderState::Taken => return,
    };
    if let Poll::Ready(result) = result {
        *state = ReaderState::Complete(result);
    }
}

fn take_reader_result(state: &mut ReaderState) -> Result<LimitedBytes, ProcessRunError> {
    let result = std::mem::replace(state, ReaderState::Taken);
    match result {
        ReaderState::Complete(Ok(result)) => result.map_err(ProcessRunError::OutputRead),
        ReaderState::Complete(Err(error)) => Err(ProcessRunError::OutputTask(error.to_string())),
        ReaderState::Pending(_) | ReaderState::Taken => {
            unreachable!("reader join only completes after both tasks")
        }
    }
}

async fn finish_readers_after_termination(readers: &mut ReaderTasks) -> bool {
    match tokio::time::timeout(TERMINATED_PIPE_DRAIN_TIMEOUT, readers.join()).await {
        Ok(Ok(_)) => true,
        Ok(Err(_)) => false,
        Err(_) => {
            readers.abort_pending();
            // Aborted Tokio tasks settle on the next poll. `join` only polls
            // handles that are still pending, preserving already completed values.
            let _ = readers.join().await;
            false
        }
    }
}

async fn read_bounded(
    mut pipe: impl AsyncRead + Unpin,
    maximum: usize,
) -> io::Result<LimitedBytes> {
    let mut bytes = Vec::with_capacity(maximum.min(8 * 1024));
    let mut buffer = [0_u8; 4096];
    let mut exceeded = false;
    loop {
        let read = pipe.read(&mut buffer).await?;
        if read == 0 {
            return Ok(LimitedBytes { bytes, exceeded });
        }
        let remaining = maximum.saturating_sub(bytes.len());
        exceeded |= read > remaining;
        bytes.extend_from_slice(&buffer[..read.min(remaining)]);
    }
}

async fn terminate_and_reap(child: &mut Child) -> Result<ExitStatus, String> {
    match child.kill().await {
        Ok(()) => child
            .wait()
            .await
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

#[cfg(all(test, feature = "app", target_os = "windows"))]
pub(in crate::launcher) async fn capture_bounded(
    command: std::process::Command,
    timeout: Duration,
    stdout_limit: usize,
) -> Result<Vec<u8>, String> {
    let output = run_bounded(Command::from(command), timeout, stdout_limit, 8 * 1024)
        .await
        .map_err(|error| error.message("ショートカット確認", timeout))?;
    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr)
            .trim()
            .chars()
            .take(400)
            .collect::<String>();
        return Err(format!(
            "ショートカットを解決できませんでした: {} {details}",
            output.status
        ));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;
    use tokio::io::{duplex, AsyncWriteExt};

    fn command(script: &str) -> Command {
        #[cfg(unix)]
        let mut command = Command::new("sh");
        #[cfg(windows)]
        let mut command = Command::new("powershell.exe");
        #[cfg(unix)]
        command.args(["-c", script]);
        #[cfg(windows)]
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ]);
        command
    }

    #[tokio::test]
    async fn captures_normal_stdout_and_stderr() {
        #[cfg(unix)]
        let command = command("printf out; printf err >&2");
        #[cfg(windows)]
        let command = command("[Console]::Out.Write('out'); [Console]::Error.Write('err')");
        // Allow PowerShell cold startup; the stalled-child tests exercise short deadlines.
        let output = run_bounded(command, Duration::from_secs(5), 32, 32)
            .await
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"out");
        assert_eq!(output.stderr, b"err");
    }

    #[tokio::test]
    async fn drains_both_pipes_after_individual_limits_are_exceeded() {
        #[cfg(unix)]
        let command = command("head -c 32768 /dev/zero; head -c 32768 /dev/zero >&2");
        #[cfg(windows)]
        let command =
            command("[Console]::Out.Write('x' * 32768); [Console]::Error.Write('y' * 32768)");
        let error = run_bounded(command, Duration::from_secs(5), 17, 23)
            .await
            .unwrap_err();
        assert!(error
            .message("確認用process", Duration::from_secs(5))
            .contains("容量上限"));
        assert!(matches!(
            &error,
            ProcessRunError::OutputLimit {
                stdout: true,
                stderr: true
            }
        ));
    }

    #[tokio::test]
    async fn output_limit_accepts_exact_capacity_and_rejects_one_more_byte() {
        async fn read_payload(payload: &[u8], limit: usize) -> LimitedBytes {
            let (mut writer, reader) = duplex(payload.len().max(1));
            writer.write_all(payload).await.unwrap();
            drop(writer);
            read_bounded(reader, limit).await.unwrap()
        }

        let exact = read_payload(b"12345678", 8).await;
        assert_eq!(exact.bytes, b"12345678");
        assert!(!exact.exceeded);

        let over = read_payload(b"123456789", 8).await;
        assert_eq!(over.bytes, b"12345678");
        assert!(over.exceeded);
    }

    #[tokio::test]
    async fn reader_error_cleanup_does_not_repoll_completed_handles() {
        let mut readers = ReaderTasks {
            stdout: ReaderState::Pending(tokio::spawn(async {
                Err(io::Error::other("fixture read failure"))
            })),
            stderr: ReaderState::Pending(tokio::spawn(async {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(LimitedBytes {
                    bytes: Vec::new(),
                    exceeded: false,
                })
            })),
        };

        assert!(!tokio::time::timeout(
            Duration::from_secs(1),
            finish_readers_after_termination(&mut readers)
        )
        .await
        .expect("reader error cleanup should finish"));
        assert!(matches!(readers.stdout, ReaderState::Taken));
        assert!(matches!(readers.stderr, ReaderState::Taken));
    }

    #[tokio::test(start_paused = true)]
    async fn interrupted_partial_join_preserves_completion_through_cleanup_timeout() {
        let (stdout_writer, stdout) = duplex(8);
        let (mut stderr_writer, stderr) = duplex(8);
        drop(stdout_writer);
        let mut readers = ReaderTasks {
            stdout: ReaderState::Pending(tokio::spawn(read_bounded(stdout, 8))),
            stderr: ReaderState::Pending(tokio::spawn(read_bounded(stderr, 8))),
        };

        // Unlike a child still running, this polls the join before cancellation:
        // stdout reaches EOF while the other pipe remains open indefinitely.
        assert!(
            tokio::time::timeout(Duration::from_millis(1), readers.join())
                .await
                .is_err()
        );
        assert!(matches!(readers.stdout, ReaderState::Complete(Ok(Ok(_)))));
        assert!(matches!(readers.stderr, ReaderState::Pending(_)));

        let started = tokio::time::Instant::now();
        assert!(!finish_readers_after_termination(&mut readers).await);
        assert!(started.elapsed() >= TERMINATED_PIPE_DRAIN_TIMEOUT);
        assert!(matches!(readers.stdout, ReaderState::Taken));
        assert!(matches!(readers.stderr, ReaderState::Taken));
        assert!(stderr_writer.write_all(b"x").await.is_err());
    }

    #[tokio::test]
    async fn timeout_kills_and_reaps_child() {
        #[cfg(unix)]
        let command = command("exec 1>&-; exec sleep 30");
        #[cfg(windows)]
        let command = command("[Console]::OpenStandardOutput().Close(); Start-Sleep -Seconds 30");
        let started = tokio::time::Instant::now();
        let error = run_bounded(command, Duration::from_millis(50), 16, 16)
            .await
            .unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(matches!(
            &error,
            ProcessRunError::TimedOut {
                termination: Ok(_),
                ..
            }
        ));
        assert!(error
            .message("確認用process", Duration::from_millis(50))
            .contains("タイムアウト"));
    }

    #[tokio::test]
    async fn reports_termination_failure_after_runner_timeout() {
        #[cfg(unix)]
        let command = command("exec sleep 30");
        #[cfg(windows)]
        let command = command("Start-Sleep -Seconds 30");
        let terminate: Terminator = Arc::new(|child| {
            Box::pin(async move {
                terminate_and_reap(child).await?;
                Err("fixture termination failure".to_string())
            })
        });
        let error =
            run_bounded_with_terminator(command, Duration::from_millis(50), 16, 16, terminate)
                .await
                .unwrap_err();
        assert!(
            matches!(error, ProcessRunError::TimedOut { termination: Err(reason), .. } if reason == "fixture termination failure")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancelling_runner_kills_and_reaps_child() {
        let path =
            std::env::temp_dir().join(format!("rice-process-cancel-{}.pid", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut command = StdCommand::new("sh");
        command.args([
            "-c",
            "exec 1>&-; echo $$ > \"$RICE_TEST_PID_FILE\"; exec sleep 30",
        ]);
        command.env("RICE_TEST_PID_FILE", &path);
        let task = tokio::spawn(run_bounded(
            Command::from(command),
            Duration::from_secs(30),
            16,
            16,
        ));
        tokio::time::timeout(Duration::from_secs(2), async {
            while !path.is_file() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("child should publish its pid");
        tokio::time::sleep(Duration::from_millis(100)).await;
        let pid = std::fs::read_to_string(&path)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        task.abort();
        let _ = task.await;
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("cancelled child should be reaped");
        let _ = std::fs::remove_file(path);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn cancelling_runner_kills_and_reaps_child() {
        let path =
            std::env::temp_dir().join(format!("rice-process-cancel-{}.pid", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut command = StdCommand::new("powershell.exe");
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::OpenStandardOutput().Close(); [IO.File]::WriteAllText($env:RICE_TEST_PID_FILE, [string]$PID); Start-Sleep -Seconds 30",
        ]);
        command.env("RICE_TEST_PID_FILE", &path);
        let task = tokio::spawn(run_bounded(
            Command::from(command),
            Duration::from_secs(30),
            16,
            16,
        ));
        tokio::time::timeout(Duration::from_secs(5), async {
            while !path.is_file() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("child should publish its pid");
        tokio::time::sleep(Duration::from_millis(100)).await;
        let pid = std::fs::read_to_string(&path)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        task.abort();
        let _ = task.await;
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let mut query = Command::new("tasklist.exe");
                query.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
                let output = run_bounded(query, Duration::from_secs(2), 4096, 2048)
                    .await
                    .expect("query child process status");
                if !String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("cancelled child should be reaped");
        let _ = std::fs::remove_file(path);
    }
}
