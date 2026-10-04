use super::SettingsWriterGuard;
use crate::settings::{
    tests::{cleanup, settings_path_for_test},
    AppSettings, SettingsStore,
};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const CHILD_TEST: &str = "settings::writer::tests::writer_process_fixture";

struct TestChild(Child);

impl TestChild {
    fn spawn(path: &Path, role: &str) -> Self {
        Self(
            Command::new(std::env::current_exe().expect("test binary"))
                .args(["--exact", CHILD_TEST, "--nocapture"])
                .env("RICE_WRITER_TEST_PATH", path)
                .env("RICE_WRITER_TEST_ROLE", role)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("spawn independent settings process"),
        )
    }

    fn ready(&mut self) {
        let stdout = self.0.stdout.take().expect("child stdout");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if line
                    .expect("read child output")
                    .contains("RICE_WRITER_READY")
                {
                    let _ = tx.send(());
                }
            }
        });
        rx.recv_timeout(Duration::from_secs(20))
            .expect("writer readiness deadline");
    }

    fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = self.0.try_wait().expect("poll child") {
                return status;
            }
            assert!(Instant::now() < deadline, "settings process exit deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for TestChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn writer_process_fixture() {
    let Some(role) = std::env::var_os("RICE_WRITER_TEST_ROLE") else {
        return;
    };
    let path =
        std::path::PathBuf::from(std::env::var_os("RICE_WRITER_TEST_PATH").expect("child path"));
    if role == "contender" {
        let before = fs::read(&path).expect("existing settings");
        let error =
            SettingsWriterGuard::acquire(&path).expect_err("second process rejected before load");
        assert!(error.to_string().contains("既に起動"));
        assert_eq!(fs::read(&path).expect("settings unchanged"), before);
        return;
    }
    let guard = SettingsWriterGuard::acquire(&path).expect("process owns settings");
    guard.require_path(&path).expect("matching path");
    let mut loaded = SettingsStore::load_from_path(&path)
        .expect("load only after ownership")
        .settings;
    if role == "owner" {
        loaded.speech.blocked_words = vec!["preserve-other-section".into()];
        SettingsStore::save_to_path(&path, &loaded).expect("first process save");
        println!("RICE_WRITER_READY");
        std::io::stdout().flush().expect("readiness flush");
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .expect("release signal");
    } else {
        assert_eq!(loaded.speech.blocked_words, ["preserve-other-section"]);
        loaded.twitch.channel_login = "successor".into();
        SettingsStore::save_to_path(&path, &loaded).expect("new owner fresh snapshot save");
    }
}

fn two_process_scenario(crash: bool) {
    let path = settings_path_for_test(if crash {
        "writer-crash"
    } else {
        "writer-normal"
    });
    SettingsStore::save_to_path(&path, &AppSettings::default()).expect("initial fixture");
    let mut owner = TestChild::spawn(&path, "owner");
    owner.ready();
    let bytes = fs::read(&path).expect("first owner saved");
    let mut contender = TestChild::spawn(&path, "contender");
    assert!(
        contender.wait().success(),
        "contender verifies fail-closed rejection"
    );
    assert_eq!(fs::read(&path).expect("no lost update"), bytes);
    if crash {
        owner.0.kill().expect("simulate owner crash");
        assert!(!owner.wait().success());
    } else {
        owner
            .0
            .stdin
            .take()
            .expect("owner stdin")
            .write_all(b"release\n")
            .expect("release owner");
        assert!(owner.wait().success());
    }
    assert!(
        path.with_file_name("settings.writer.lock").exists(),
        "stable inode is not deleted on exit"
    );
    let mut successor = TestChild::spawn(&path, "successor");
    assert!(
        successor.wait().success(),
        "OS releases ownership after exit/crash"
    );
    let loaded = SettingsStore::load_from_path(&path)
        .expect("read result")
        .settings;
    assert_eq!(loaded.twitch.channel_login, "successor");
    assert_eq!(loaded.speech.blocked_words, ["preserve-other-section"]);
    drop(successor);
    drop(contender);
    drop(owner);
    cleanup(&path);
}

#[test]
fn two_process_owner_exit_preserves_other_sections() {
    two_process_scenario(false);
}

#[test]
fn two_process_owner_crash_releases_lock() {
    two_process_scenario(true);
}

#[test]
fn ownership_cannot_be_used_for_another_settings_path() {
    let path = settings_path_for_test("writer-path");
    let guard = SettingsWriterGuard::acquire(&path).expect("lock");
    assert!(guard
        .require_path(&path.with_file_name("another.json"))
        .is_err());
    assert!(SettingsWriterGuard::acquire(&path).is_err());
    drop(guard);
    SettingsWriterGuard::acquire(&path).expect("released when dropped");
    cleanup(&path);
}

#[cfg(unix)]
#[test]
fn lock_file_is_private_and_rejects_links() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let path = settings_path_for_test("writer-links");
    let lock_path = path.with_file_name("settings.writer.lock");
    let guard = SettingsWriterGuard::acquire(&path).expect("lock");
    assert_eq!(
        fs::metadata(&lock_path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    drop(guard);
    fs::remove_file(&lock_path).expect("remove test lock only when unowned");
    let target = path.with_file_name("outside-fixture");
    fs::write(&target, "unchanged").expect("fixture");
    symlink(&target, &lock_path).expect("test symlink");
    assert!(SettingsWriterGuard::acquire(&path).is_err());
    fs::remove_file(&lock_path).expect("test symlink");
    fs::hard_link(&target, &lock_path).expect("test hardlink");
    assert!(SettingsWriterGuard::acquire(&path).is_err());
    assert_eq!(
        fs::read_to_string(&target).expect("unchanged target"),
        "unchanged"
    );
    cleanup(&path);
}
