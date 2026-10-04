use std::path::Path;
use std::process::{Command, Stdio};

pub(in crate::launcher) fn spawn_application(target: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;

    // CREATE_NO_WINDOW prevents console applications from flashing a console window.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let is_shortcut = target
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"));
    let mut command = if is_shortcut {
        let mut command = Command::new("explorer.exe");
        command.arg(target);
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

    command
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}
