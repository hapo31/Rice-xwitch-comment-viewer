//! Linux default-browser launching through the command candidates supplied by `open`.

pub(crate) fn open_system_url(url: &str) -> anyhow::Result<()> {
    let mut errors = Vec::new();

    for mut command in open::commands(url) {
        let program = command.get_program().to_string_lossy().into_owned();
        match command.output() {
            Ok(output) if output.status.success() => return Ok(()),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let detail = if stderr.is_empty() {
                    format!("終了コード {}", output.status)
                } else {
                    stderr
                };
                errors.push(format!("{program}: {detail}"));
            }
            Err(error) => errors.push(format!("{program}: {error}")),
        }
    }

    if errors.is_empty() {
        return Err(anyhow::anyhow!("既定ブラウザを開く方法が見つかりません。"));
    }

    Err(anyhow::anyhow!(errors.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::open_system_url;
    use std::{os::unix::fs::PermissionsExt, path::Path, process::Command};

    const TEST_URL: &str = "https://www.twitch.tv/activate";

    #[test]
    fn fake_opener_worker() {
        let Ok(mode) = std::env::var("RICE_OPENER_TEST_MODE") else {
            return;
        };

        let temp = tempfile::tempdir().expect("temporary directory");
        std::env::set_var("WSL_DISTRO_NAME", "rice-opener-test");
        let commands = open::commands(TEST_URL);
        let programs = commands
            .iter()
            .map(|command| {
                Path::new(command.get_program())
                    .file_name()
                    .expect("opener command has a program name")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        assert!(
            programs.len() >= 2,
            "WSL opener should have fallback candidates"
        );
        assert_ne!(programs[0], programs[1]);

        let marker = temp.path().join("calls.txt");
        for program in &programs {
            let script_path = temp.path().join(program);
            if script_path.exists() {
                continue;
            }
            std::fs::write(
                &script_path,
                "#!/bin/sh\nname=${0##*/}\nprintf '%s\\n' \"$name\" >> \"$RICE_OPENER_MARKER\"\nif [ \"$name\" = \"$RICE_OPENER_SUCCESS\" ]; then exit 0; fi\necho \"fake failure: $name\" >&2\nexit 19\n",
            )
            .expect("write fake opener executable");
            let mut permissions = std::fs::metadata(&script_path)
                .expect("fake opener metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(script_path, permissions)
                .expect("make fake opener executable");
        }

        std::env::set_var("PATH", temp.path());
        std::env::set_var("RICE_OPENER_MARKER", &marker);
        let success_name = match mode.as_str() {
            "first-success" => programs[0].as_str(),
            "second-success" => programs[1].as_str(),
            "all-fail" => "no-fake-opener-succeeds",
            _ => panic!("unknown opener test mode"),
        };
        std::env::set_var("RICE_OPENER_SUCCESS", success_name);

        let result = open_system_url(TEST_URL);
        let calls = std::fs::read_to_string(marker)
            .expect("fake opener call marker")
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        match mode.as_str() {
            "first-success" => {
                assert!(result.is_ok(), "{result:?}");
                assert_eq!(calls.as_slice(), &programs[..1]);
            }
            "second-success" => {
                assert!(result.is_ok(), "{result:?}");
                assert_eq!(calls.as_slice(), &programs[..2]);
            }
            "all-fail" => {
                let error = result.expect_err("all fake openers should fail");
                assert!(error.to_string().contains("fake failure"));
                assert_eq!(calls, programs);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn tries_next_candidate_on_nonzero_and_preserves_failure_details() {
        let test_executable = std::env::current_exe().expect("test executable path");

        for mode in ["first-success", "second-success", "all-fail"] {
            let output = Command::new(&test_executable)
                .args([
                    "--exact",
                    "external_url::tests::fake_opener_worker",
                    "--nocapture",
                ])
                .env("RICE_OPENER_TEST_MODE", mode)
                .output()
                .expect("run isolated fake opener test process");
            assert!(
                output.status.success(),
                "{mode}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
