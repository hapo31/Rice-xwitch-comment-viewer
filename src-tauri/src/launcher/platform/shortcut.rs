//! Bounded shortcut metadata/header policy; tests do not need Windows/COM.
use serde::Deserialize;

pub(in crate::launcher) const MAX_SHORTCUT_FILE_BYTES: u64 = 1024 * 1024;
#[cfg(test)]
pub(in crate::launcher) const MAX_SHORTCUT_JSON_BYTES: usize = 96 * 1024;
pub(in crate::launcher) const SHORTCUT_REPAIR: &str = "ショートカットのプロパティでリンク先・作業フォルダーを修正するか、正しいアプリを再登録してください。";

/// Inspect the fixed MS-SHLLINK header before asking COM to read the file.
/// Do not activate advertised MSI links or silently ignore another-user flags.
pub(in crate::launcher) fn validate_header(header: &[u8], file_bytes: u64) -> Result<(), String> {
    const CLSID: [u8; 16] = [1, 0x14, 2, 0, 0, 0, 0, 0, 0xc0, 0, 0, 0, 0, 0, 0, 0x46];
    if file_bytes > MAX_SHORTCUT_FILE_BYTES {
        return Err(
            "ショートカットは1MiB以内にしてください。正しいアプリを再登録してください。".into(),
        );
    }
    if header.len() != 76 || header[..4] != 76u32.to_le_bytes() || header[4..20] != CLSID {
        return Err(format!(
            "ショートカットの形式が壊れているか対応していません。{SHORTCUT_REPAIR}"
        ));
    }
    let flags = u32::from_le_bytes(header[20..24].try_into().expect("fixed header"));
    if flags & 0x1000 != 0 {
        return Err("Windows Installerの特殊ショートカットは対応していません。スタートメニューから手動で起動するか、通常の実行ファイルを登録してください。".into());
    }
    if flags & 0x2000 != 0 {
        return Err("管理者・別ユーザーとして実行するショートカットは自動起動できません。Windowsから手動で起動してください。昇格要求は送っていません。".into());
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::launcher) struct ShortcutMetadata {
    pub target: String,
    pub arguments: String,
    pub working_directory: String,
    pub icon_source: String,
}

impl ShortcutMetadata {
    #[cfg(test)]
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_SHORTCUT_JSON_BYTES {
            return Err(format!(
                "ショートカットの内容が大きすぎます。{SHORTCUT_REPAIR}"
            ));
        }
        let result: Self = serde_json::from_slice(bytes)
            .map_err(|_| format!("ショートカットの内容を確認できません。{SHORTCUT_REPAIR}"))?;
        result.validate()?;
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), String> {
        for field in [&self.target, &self.working_directory, &self.icon_source] {
            if field.len() > 4096 || field.chars().any(char::is_control) {
                return Err(format!("ショートカットのパスは制御文字を含まない4096バイト以内にしてください。{SHORTCUT_REPAIR}"));
            }
        }
        if self.arguments.len() > 64 * 1024
            || self.arguments.encode_utf16().count() > 16 * 1024
            || self
                .arguments
                .chars()
                .any(|value| value.is_control() && value != '\t')
        {
            return Err(format!(
                "ショートカットの引数が長すぎるか制御文字を含んでいます。{SHORTCUT_REPAIR}"
            ));
        }
        if !windows_absolute(&self.target) || !self.target.to_ascii_lowercase().ends_with(".exe") {
            return Err(format!("通常の実行ファイルを参照するショートカットだけに対応しています。URL・仮想フォルダー・入れ子のリンクは起動できません。{SHORTCUT_REPAIR}"));
        }
        if !self.working_directory.is_empty() && !windows_absolute(&self.working_directory) {
            return Err(format!(
                "ショートカットの作業フォルダーは絶対パスにしてください。{SHORTCUT_REPAIR}"
            ));
        }
        Ok(())
    }
}

fn windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    (bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\'))
        || (path.starts_with("\\\\")
            && path[2..]
                .split('\\')
                .filter(|part| !part.is_empty())
                .count()
                >= 2)
}

pub(in crate::launcher) fn spawn_failure_message(code: Option<i32>, details: &str) -> String {
    match code {
        Some(740) => "このアプリは管理者権限が必要です。Windowsから手動で起動してください。昇格要求は送っていません。".into(),
        Some(5) => "実行権限がありません。ファイルのアクセス権を確認するか、Windowsから手動で起動してください。".into(),
        _ => format!("起動先のプロセスを生成できませんでした。ファイルの場所・実行権限を確認し、必要なら正しいアプリを再登録してください: {details}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_error_codes_do_not_trigger_elevation_or_depend_on_os_message_language() {
        for details in ["Elevation required", "昇格が必要", "unrelated"] {
            assert!(spawn_failure_message(Some(740), details).contains("昇格要求は送っていません"));
            assert!(spawn_failure_message(Some(5), details).contains("実行権限がありません"));
        }
        assert!(spawn_failure_message(Some(2), "missing").contains("再登録"));
    }

    fn header(flags: u32) -> Vec<u8> {
        let mut bytes = vec![0; 76];
        bytes[..4].copy_from_slice(&76u32.to_le_bytes());
        bytes[4..20].copy_from_slice(&[1, 0x14, 2, 0, 0, 0, 0, 0, 0xc0, 0, 0, 0, 0, 0, 0, 0x46]);
        bytes[20..24].copy_from_slice(&flags.to_le_bytes());
        bytes
    }

    #[test]
    fn header_rejects_corrupt_oversized_installer_and_other_user_links() {
        assert!(validate_header(&header(0x9b), 1000).is_ok());
        assert!(validate_header(&header(0), MAX_SHORTCUT_FILE_BYTES).is_ok());
        assert!(validate_header(&header(0), MAX_SHORTCUT_FILE_BYTES + 1).is_err());
        assert!(validate_header(&[], 0).is_err());
        assert!(validate_header(&header(0)[..75], 75).is_err());
        let mut wrong = header(0);
        wrong[4] ^= 1;
        assert!(validate_header(&wrong, 76).is_err());
        assert!(validate_header(&header(0x1000), 76)
            .unwrap_err()
            .contains("Installer"));
        assert!(validate_header(&header(0x2000), 76)
            .unwrap_err()
            .contains("昇格要求は送っていません"));
    }

    fn metadata() -> serde_json::Value {
        serde_json::json!({"target": "C:\\配信 アプリ\\probe.exe", "arguments": "\"日本語 空白\" & | < > ^ % $", "workingDirectory": "C:\\別の 作業場所", "iconSource": ""})
    }

    #[test]
    fn json_preserves_raw_arguments_and_unicode_without_shell_interpretation() {
        let source = metadata();
        let decoded = ShortcutMetadata::decode(&serde_json::to_vec(&source).unwrap()).unwrap();
        assert_eq!(decoded.target, source["target"].as_str().unwrap());
        assert_eq!(decoded.arguments, source["arguments"].as_str().unwrap());
        assert_eq!(
            decoded.working_directory,
            source["workingDirectory"].as_str().unwrap()
        );
        let mut unc = source;
        unc["target"] = "\\\\server\\share\\app.EXE".into();
        unc["workingDirectory"] = "".into();
        assert!(ShortcutMetadata::decode(&serde_json::to_vec(&unc).unwrap()).is_ok());
    }

    #[test]
    fn unsupported_targets_fields_paths_and_json_limits_fail_closed() {
        for target in [
            "",
            "relative.exe",
            "C:app.exe",
            "https://example.com/app.exe",
            "C:\\folder",
            "C:\\app.lnk",
            "C:\\app.exe\n",
        ] {
            let mut source = metadata();
            source["target"] = target.into();
            assert!(
                ShortcutMetadata::decode(&serde_json::to_vec(&source).unwrap()).is_err(),
                "{target}"
            );
        }
        for (field, value) in [
            ("unknown", "x".into()),
            ("workingDirectory", "relative".into()),
            ("target", format!("C:\\{}.exe", "x".repeat(4096))),
            ("arguments", "x".repeat(16385)),
            ("arguments", "😀".repeat(8193)),
            ("arguments", "a\0b".into()),
            ("iconSource", "x\ny".into()),
        ] {
            let mut source = metadata();
            source[field] = value.into();
            assert!(
                ShortcutMetadata::decode(&serde_json::to_vec(&source).unwrap()).is_err(),
                "{field}"
            );
        }
        assert!(ShortcutMetadata::decode(&vec![b' '; MAX_SHORTCUT_JSON_BYTES + 1]).is_err());
        assert!(ShortcutMetadata::decode(b"[]").is_err());
        let mut source = metadata();
        source["arguments"] = "😀".repeat(8192).into();
        assert!(ShortcutMetadata::decode(&serde_json::to_vec(&source).unwrap()).is_ok());
    }
}
