//! Allocation-conscious JSON/storage limits shared by request preflight and
//! settings persistence. Errors never echo untrusted payloads or paths.
use serde::Serialize;
use std::io::{self, Read, Write};
use std::path::Path;

pub(crate) const MAX_SETTINGS_JSON_BYTES: usize = 8 * 1024 * 1024;
const MAX_REQUEST_NODES: usize = 4096;
const MAX_REQUEST_DEPTH: usize = 16;

#[derive(Debug, thiserror::Error)]
#[error("{label}は最大 {maximum} バイトです。項目数や入力内容を減らしてください。")]
pub(crate) struct SizeLimitExceeded {
    label: &'static str,
    maximum: usize,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum BoundedReadError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    TooLarge(#[from] SizeLimitExceeded),
    #[error("設定ファイルが正しいUTF-8ではありません。")]
    Encoding(#[from] std::string::FromUtf8Error),
}

struct LimitedWriter<W> {
    inner: W,
    written: usize,
    maximum: usize,
    exceeded: bool,
}

impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .written
            .checked_add(bytes.len())
            .is_none_or(|next| next > self.maximum)
        {
            self.exceeded = true;
            return Err(io::Error::other("JSON size limit exceeded"));
        }
        let count = self.inner.write(bytes)?;
        self.written += count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) fn serialize_bounded<T: Serialize + ?Sized>(
    value: &T,
    maximum: usize,
    label: &'static str,
) -> anyhow::Result<Vec<u8>> {
    let mut writer = LimitedWriter {
        inner: Vec::with_capacity(maximum.min(8192)),
        written: 0,
        maximum,
        exceeded: false,
    };
    let result = serde_json::to_writer_pretty(&mut writer, value);
    if writer.exceeded {
        return Err(SizeLimitExceeded { label, maximum }.into());
    }
    result?;
    Ok(writer.inner)
}

pub(crate) fn validate_json_request(
    value: &serde_json::Value,
    maximum: usize,
) -> Result<(), String> {
    // Tauri has already parsed its transport into a borrowed Value. Bound the
    // tree before any application-owned DTO/Value clone, including tiny-node
    // amplification and nesting, without allocating a serialized String.
    fn visit(value: &serde_json::Value, depth: usize, remaining: &mut usize) -> Result<(), String> {
        if depth > MAX_REQUEST_DEPTH || *remaining == 0 {
            return Err("要求の構造が大きすぎます。項目数や入力内容を減らしてください。".into());
        }
        *remaining -= 1;
        match value {
            serde_json::Value::Array(values) => {
                for value in values {
                    visit(value, depth + 1, remaining)?;
                }
            }
            serde_json::Value::Object(values) => {
                for value in values.values() {
                    visit(value, depth + 1, remaining)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut remaining = MAX_REQUEST_NODES;
    visit(value, 0, &mut remaining)?;
    let mut writer = LimitedWriter {
        inner: io::sink(),
        written: 0,
        maximum,
        exceeded: false,
    };
    let result = serde_json::to_writer(&mut writer, value);
    if writer.exceeded {
        return Err(SizeLimitExceeded {
            label: "要求のJSON",
            maximum,
        }
        .to_string());
    }
    result.map_err(|_| "要求のJSONを確認できません。入力内容を見直してください。".into())
}

pub(crate) fn read_bounded(path: &Path, maximum: usize) -> Result<String, BoundedReadError> {
    let file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    if length > maximum as u64 {
        return Err(SizeLimitExceeded {
            label: "設定ファイル",
            maximum,
        }
        .into());
    }
    let mut reader = file.take(maximum as u64 + 1);
    let mut bytes = Vec::with_capacity(length as usize);
    let mut chunk = [0u8; 8192];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        check_bytes(bytes.len() + count, maximum, "設定ファイル")?;
        // Handle file growth without geometric over-allocation beyond the cap.
        if bytes.capacity() < bytes.len() + count {
            bytes.reserve_exact(count);
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(String::from_utf8(bytes)?)
}

pub(crate) fn check_bytes(
    length: usize,
    maximum: usize,
    label: &'static str,
) -> Result<(), SizeLimitExceeded> {
    if length > maximum {
        return Err(SizeLimitExceeded { label, maximum });
    }
    Ok(())
}

#[cfg(feature = "app")]
pub(crate) fn request_json<'a>(
    request: &'a tauri::ipc::Request<'_>,
) -> Result<&'a serde_json::Value, String> {
    match request.body() {
        tauri::ipc::InvokeBody::Json(value) => Ok(value),
        _ => Err("JSON形式で要求してください。入力内容を見直してください。".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_serialization_rejects_before_extending_its_buffer() {
        assert_eq!(
            serialize_bounded(&"hello", 7, "設定").unwrap(),
            b"\"hello\""
        );
        let error = serialize_bounded(&"hello", 6, "設定").unwrap_err();
        assert!(error.downcast_ref::<SizeLimitExceeded>().is_some());
        assert!(!error.to_string().contains("hello"));
    }
    #[test]
    fn request_size_includes_escaping_and_rejects_node_and_depth_amplification() {
        let value = serde_json::json!("\n\n");
        validate_json_request(&value, 6).unwrap();
        assert!(validate_json_request(&value, 5).is_err());
        validate_json_request(
            &serde_json::json!(vec![0; MAX_REQUEST_NODES - 1]),
            MAX_SETTINGS_JSON_BYTES,
        )
        .unwrap();
        assert!(validate_json_request(
            &serde_json::json!(vec![0; MAX_REQUEST_NODES]),
            MAX_SETTINGS_JSON_BYTES
        )
        .is_err());
        let mut value = serde_json::Value::Null;
        for _ in 0..MAX_REQUEST_DEPTH {
            value = serde_json::json!([value]);
        }
        validate_json_request(&value, MAX_SETTINGS_JSON_BYTES).unwrap();
        value = serde_json::json!([value]);
        assert!(validate_json_request(&value, MAX_SETTINGS_JSON_BYTES).is_err());
    }
}

#[cfg(test)]
pub(crate) mod allocation;
