use super::shortcut;
use crate::launcher::ports::LaunchContext;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

pub(in crate::launcher) fn spawn_application(
    target: &Path,
    context: &LaunchContext,
) -> Result<(), String> {
    context.ensure_active()?;
    // CREATE_NO_WINDOW prevents console applications from flashing a console window.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let is_shortcut = target
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"));
    let mut command = if is_shortcut {
        let resolved = shortcut::resolve(target)?;
        // No Explorer/ShellExecute or shell interpretation. Only the freshly
        // verified executable, literal Windows argument tail and validated cwd.
        let mut command = Command::new(&resolved.executable);
        if !resolved.metadata.arguments.is_empty() {
            command.raw_arg(&resolved.metadata.arguments);
        }
        command.current_dir(&resolved.working_directory);
        command
    } else {
        // The target is passed directly to CreateProcess without a command shell. No user
        // controlled text is interpreted as command-line arguments.
        let mut command = Command::new(target);
        if let Some(parent) = target.parent() {
            command.current_dir(parent);
        }
        command
    };

    // Timeout/cancellation during COM/FS work prevents a later process start.
    context.ensure_active()?;
    command
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            super::super::shortcut::spawn_failure_message(error.raw_os_error(), &error.to_string())
        })
}

#[cfg(test)]
mod tests;
