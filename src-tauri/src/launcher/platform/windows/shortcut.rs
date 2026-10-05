//! Unicode COM boundary. No WSH ANSI conversion, shell activation or Resolve.
use super::super::shortcut::{
    validate_header, ShortcutMetadata, MAX_SHORTCUT_FILE_BYTES, SHORTCUT_REPAIR,
};
use super::super::target::FileTargetResolver;
use crate::launcher::ports::ApplicationTargetResolver;
use std::fs::File;
use std::io::Read;
use std::marker::PhantomData;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use windows::core::{Interface, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, STGM_READ,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink, SLGP_RAWPATH};

pub(super) struct ResolvedShortcut {
    pub executable: PathBuf,
    pub working_directory: PathBuf,
    pub metadata: ShortcutMetadata,
}

/// Balance even S_FALSE; all interfaces are dropped before this same-thread
/// apartment guard. Never marshal an interface between worker threads.
pub(super) struct Apartment(PhantomData<Rc<()>>);
impl Apartment {
    pub fn init() -> windows::core::Result<Self> {
        // SAFETY: reserved pointer is null, and this guard balances successful
        // initialization on the calling thread. Incompatible apartments fail.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self(PhantomData))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: the !Send/!Sync guard cannot move across threads; init
        // succeeded here (including S_FALSE), requiring exactly one balance.
        unsafe {
            CoUninitialize();
        }
    }
}
pub(super) fn new_link(_apartment: &Apartment) -> windows::core::Result<IShellLinkW> {
    // SAFETY: OS ShellLink CLSID, no aggregation, apartment initialized by caller.
    unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
}
pub(super) fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn native_error(error: windows::core::Error) -> String {
    format!("ショートカットの内容を読み取れませんでした: {error}。{SHORTCUT_REPAIR}")
}
fn read_string(
    limit: usize,
    read: impl FnOnce(&mut [u16]) -> windows::core::Result<()>,
) -> Result<String, String> {
    // One extra payload unit detects truncation at our boundary, plus NUL.
    let mut buffer = vec![0u16; limit + 2];
    read(&mut buffer).map_err(native_error)?;
    let length = buffer
        .iter()
        .position(|unit| *unit == 0)
        .ok_or_else(|| format!("ショートカットの文字列終端を確認できません。{SHORTCUT_REPAIR}"))?;
    if length > limit {
        return Err(format!(
            "ショートカットの文字列が上限を超えています。{SHORTCUT_REPAIR}"
        ));
    }
    String::from_utf16(&buffer[..length])
        .map_err(|_| format!("ショートカットのUnicode文字列が壊れています。{SHORTCUT_REPAIR}"))
}
fn expand_environment(value: String) -> String {
    let mut result = String::new();
    let mut rest = value.as_str();
    while let Some(start) = rest.find('%') {
        result.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest[1..].find('%').map(|index| index + 1) else {
            break;
        };
        let token = &rest[..=end];
        let name = &rest[1..end];
        result.push_str(&std::env::var(name).unwrap_or_else(|_| token.to_owned()));
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    result
}

fn read_metadata(path: &Path) -> Result<ShortcutMetadata, String> {
    let _apartment = Apartment::init().map_err(native_error)?;
    let link = new_link(&_apartment).map_err(native_error)?;
    let persist: IPersistFile = link.cast().map_err(native_error)?;
    let path = wide_path(path);
    // SAFETY: live NUL-terminated absolute UTF-16 path, OS-owned interface;
    // Load is read-only. We never call Resolve, Save, ShellExecute or Activate.
    unsafe { persist.Load(PCWSTR(path.as_ptr()), STGM_READ) }.map_err(native_error)?;
    // GetPath documents MAX_PATH. Reject its last payload slot rather than
    // accepting a silently truncated target, even if that prefix exists.
    let target = read_string(258, |buffer| unsafe {
        link.GetPath(buffer, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)
    })?;
    let arguments = read_string(16 * 1024, |buffer| unsafe { link.GetArguments(buffer) })?;
    let working_directory =
        read_string(4096, |buffer| unsafe { link.GetWorkingDirectory(buffer) })?;
    let mut icon_index = 0;
    let icon_source = read_string(4096, |buffer| unsafe {
        link.GetIconLocation(buffer, &mut icon_index)
    })?;
    let metadata = ShortcutMetadata {
        target: expand_environment(target),
        arguments,
        working_directory: expand_environment(working_directory),
        icon_source: expand_environment(icon_source),
    };
    metadata.validate()?;
    Ok(metadata)
}

/// Resolve fresh on every launch; the service's bounded worker/deadline owns
/// blocking COM/FS lifetime. No link tracking, repair dialog or elevation.
pub(super) fn resolve(shortcut_path: &Path) -> Result<ResolvedShortcut, String> {
    let file = File::open(shortcut_path)
        .map_err(|error| format!("ショートカットを開けませんでした: {error}。{SHORTCUT_REPAIR}"))?;
    let file_bytes = file
        .metadata()
        .map_err(|error| format!("ショートカットの大きさを確認できませんでした: {error}"))?
        .len();
    if file_bytes > MAX_SHORTCUT_FILE_BYTES {
        return Err(
            "ショートカットは1MiB以内にしてください。正しいアプリを再登録してください。".into(),
        );
    }
    let mut header = [0u8; 76];
    file.take(76)
        .read_exact(&mut header)
        .map_err(|_| format!("ショートカットの形式が壊れています。{SHORTCUT_REPAIR}"))?;
    validate_header(&header, file_bytes)?;
    let metadata = read_metadata(shortcut_path)?;
    let executable = FileTargetResolver.resolve(&metadata.target).map_err(|_| {
        format!(
            "ショートカットのリンク先が存在しないか実行ファイルではありません。{SHORTCUT_REPAIR}"
        )
    })?;
    let working_directory = if metadata.working_directory.is_empty() {
        executable
            .parent()
            .ok_or_else(|| format!("リンク先の作業フォルダーを確認できません。{SHORTCUT_REPAIR}"))?
            .to_path_buf()
    } else {
        let path = PathBuf::from(&metadata.working_directory);
        if !path.is_dir() {
            return Err(format!(
                "ショートカットの作業フォルダーが見つかりません。{SHORTCUT_REPAIR}"
            ));
        }
        path.canonicalize()
            .map(super::super::target::normalize_canonical_path)
            .map_err(|_| {
                format!("ショートカットの作業フォルダーを確認できません。{SHORTCUT_REPAIR}")
            })?
    };
    Ok(ResolvedShortcut {
        executable,
        working_directory,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn com_strings_reject_truncation_unterminated_and_invalid_unicode() {
        for limit in [258, 4096, 16 * 1024] {
            assert_eq!(
                read_string(limit, |buffer| {
                    buffer[..limit].fill(u16::from(b'a'));
                    Ok(())
                })
                .unwrap()
                .len(),
                limit
            );
            assert!(read_string(limit, |buffer| {
                buffer[..limit + 1].fill(u16::from(b'a'));
                Ok(())
            })
            .is_err());
            assert!(read_string(limit, |buffer| {
                buffer.fill(u16::from(b'a'));
                Ok(())
            })
            .is_err());
        }
        assert!(read_string(258, |buffer| {
            buffer[0] = 0xd800;
            Ok(())
        })
        .is_err());
        assert_eq!(
            expand_environment("x%RICE_UNDEFINED_ENV_76%x%".into()),
            "x%RICE_UNDEFINED_ENV_76%x%"
        );
    }
}
