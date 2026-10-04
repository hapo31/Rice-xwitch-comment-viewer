use std::io::Read;
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

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
