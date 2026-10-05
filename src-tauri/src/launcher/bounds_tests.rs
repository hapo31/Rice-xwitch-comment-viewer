use super::*;

fn item(index: u32) -> LauncherItem {
    LauncherItem {
        id: format!("item-{index}"),
        kind: LauncherItemKind::Application,
        target: format!(r"C:\Apps\app-{index}.exe"),
        display_name: format!("App {index}"),
        icon_data_url: None,
        background_color: None,
        group_id: None,
        order: index,
    }
}

fn padded_png(file_bytes: usize, dimension: u32) -> Vec<u8> {
    padded_png_with_seed(file_bytes, dimension, 0)
}

fn padded_png_with_seed(file_bytes: usize, dimension: u32, seed: u32) -> Vec<u8> {
    let encode = |padding: usize| {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, dimension, dimension);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Sixteen);
            encoder
                .add_text_chunk("padding".into(), format!("{seed}:{}", "x".repeat(padding)))
                .unwrap();
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&vec![0; dimension as usize * dimension as usize * 8])
                .unwrap();
        }
        bytes
    };
    let base = encode(0).len();
    let bytes = encode(file_bytes.checked_sub(base).expect("fixture large enough"));
    assert_eq!(bytes.len(), file_bytes);
    bytes
}

/// Worst allowed aggregate: 200 distinct 128x128 RGBA16 PNGs with CRC-valid
/// ancillary padding, exactly 4MiB data URLs. Also exercise maximum strings.
pub(crate) fn full_quota_items() -> Vec<LauncherItem> {
    let first_encoded = ((MAX_TOTAL_ICON_BYTES
        - MAX_LAUNCHER_ITEMS * LAUNCHER_ICON_DATA_URL_PREFIX.len())
        / MAX_LAUNCHER_ITEMS)
        / 4
        * 4;
    let mut items = Vec::new();
    for index in 0..MAX_LAUNCHER_ITEMS {
        let mut item = item(index as u32);
        item.id = format!("{:0>64}", item.id);
        item.display_name = "😀".repeat(MAX_DISPLAY_NAME_CHARS);
        item.group_id = Some("😀".repeat(MAX_GROUP_CHARS));
        item.background_color = Some("#1a2B3c".into());
        item.target = format!("C:\\{}-{index}.exe", "a".repeat(630));
        item.icon_data_url = Some(format!(
            "{LAUNCHER_ICON_DATA_URL_PREFIX}{}",
            BASE64_STANDARD.encode(padded_png_with_seed(
                first_encoded / 4 * 3,
                MAX_ICON_DIMENSION,
                index as u32
            ))
        ));
        items.push(item);
    }
    let used: usize = items[..MAX_LAUNCHER_ITEMS - 1]
        .iter()
        .map(|item| item.icon_data_url.as_ref().unwrap().len())
        .sum();
    let last_encoded = MAX_TOTAL_ICON_BYTES - used - LAUNCHER_ICON_DATA_URL_PREFIX.len();
    assert_eq!(last_encoded % 4, 0);
    items.last_mut().unwrap().icon_data_url = Some(format!(
        "{LAUNCHER_ICON_DATA_URL_PREFIX}{}",
        BASE64_STANDARD.encode(padded_png_with_seed(
            last_encoded / 4 * 3,
            MAX_ICON_DIMENSION,
            199
        ))
    ));
    assert_eq!(
        items
            .iter()
            .map(|item| item.icon_data_url.as_ref().unwrap().len())
            .sum::<usize>(),
        MAX_TOTAL_ICON_BYTES
    );
    items
}

pub(crate) fn full_quota_settings() -> AppSettings {
    let mut settings = AppSettings::default();
    settings.launcher.items = full_quota_items();
    settings.launcher.items[0].order = 200;
    // Valid worst-case rule payload, never a multi-MiB single NG word.
    settings.speech.blocked_words = (0..131)
        .map(|index| format!("{index:03}{}", "x".repeat(497)))
        .collect();
    crate::settings::validation::validate_settings(&settings).unwrap();
    settings
}

#[test]
fn requests_bound_raw_duplicates_utf8_bytes_and_escaped_json_before_owned_copy() {
    let path = format!("{}.exe", "a".repeat(MAX_PATH_BYTES - 4));
    assert!(parse_add_request(&serde_json::json!({"paths": [path]})).is_ok());
    for paths in [
        vec!["a.exe".into(); 201],
        vec![format!("{}.exe", "a".repeat(MAX_PATH_BYTES - 3))],
        vec![format!("{}.exe", "😀".repeat(1024))],
        vec!["\nC:\\app.exe\n".into()],
        vec![format!("{}.exe", "a".repeat(700)); 200],
    ] {
        let request = serde_json::json!({"paths": paths});
        let error = parse_add_request(&request).unwrap_err();
        assert!(!error.contains(&"a".repeat(700)));
    }
    let at_limit = vec![path; MAX_PATHS_BYTES / MAX_PATH_BYTES];
    validate_paths(at_limit.iter().map(String::as_str)).unwrap();
    let mut above_limit = at_limit;
    above_limit.push("a.exe".into());
    assert!(validate_paths(above_limit.iter().map(String::as_str)).is_err());
    let request =
        serde_json::json!({"paths": ["a.exe"], "ignored": "x".repeat(MAX_ADD_REQUEST_BYTES)});
    assert!(parse_add_request(&request).is_err());
    assert!(parse_add_request(&serde_json::json!({"paths": []})).is_err());
    assert!(parse_add_request(&serde_json::json!({"paths": [null]})).is_err());
}

#[test]
fn extraction_output_and_file_item_deserialization_are_bounded_not_truncated() {
    assert_eq!(read_pipe_bounded(Cursor::new(b"1234"), 4).unwrap(), b"1234");
    assert_eq!(
        read_pipe_bounded(Cursor::new(b"12345"), 4)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    let valid = serde_json::to_value(item(0)).unwrap();
    assert!(serde_json::from_value::<LauncherSettings>(
        serde_json::json!({"items": vec![valid.clone(); 200]})
    )
    .is_ok());
    let mut values = vec![valid; 200];
    // The 201st element is skipped without an owned LauncherItem allocation.
    values.push(serde_json::json!({"unknown": "x".repeat(1024)}));
    assert!(
        serde_json::from_value::<LauncherSettings>(serde_json::json!({"items": values}))
            .unwrap_err()
            .to_string()
            .contains("200")
    );
}

#[test]
fn metadata_and_missing_stored_targets_are_validated_without_filesystem_access() {
    let existing = item(0); // Does not exist, on any CI platform.
    let edit = LauncherItemEdit {
        id: existing.id.clone(),
        display_name: "😀".repeat(120),
        background_color: Some("#aAbB00".into()),
        group_id: Some("😀".repeat(64)),
        order: u32::MAX,
    };
    let changed =
        apply_launcher_edits(std::slice::from_ref(&existing), vec![edit.clone()]).unwrap();
    assert_eq!(changed[0].target, existing.target);
    assert_eq!(changed[0].icon_data_url, existing.icon_data_url);
    for field in ["id", "displayName", "backgroundColor", "groupId"] {
        let mut invalid = edit.clone();
        match field {
            "id" => invalid.id = "new-id".into(),
            "displayName" => invalid.display_name.push('x'),
            "backgroundColor" => invalid.background_color = Some("red".into()),
            _ => invalid.group_id.as_mut().unwrap().push('x'),
        }
        assert!(
            apply_launcher_edits(std::slice::from_ref(&existing), vec![invalid]).is_err(),
            "{field}"
        );
    }
    assert!(apply_launcher_edits(&[existing], vec![edit.clone(), edit]).is_err());
    for id in [
        "".into(),
        "bad/id".into(),
        "😀".into(),
        "x".repeat(MAX_ID_BYTES + 1),
    ] {
        let mut invalid = item(0);
        invalid.id = id;
        assert!(validate_launcher_resources(&[invalid]).is_err());
    }
}

#[test]
fn png_encoded_file_pixels_and_combined_quota_have_explicit_boundaries() {
    let at_limit = format!(
        "{LAUNCHER_ICON_DATA_URL_PREFIX}{}",
        BASE64_STANDARD.encode(padded_png(MAX_ICON_FILE_BYTES, MAX_ICON_DIMENSION))
    );
    assert_eq!(
        at_limit.len() - LAUNCHER_ICON_DATA_URL_PREFIX.len(),
        MAX_ICON_BASE64_LENGTH
    );
    assert!(valid_icon_data_url(&at_limit));
    for bytes in [
        padded_png(MAX_ICON_FILE_BYTES + 1, 1),
        padded_png(2000, MAX_ICON_DIMENSION + 1),
    ] {
        assert!(!valid_icon_data_url(&format!(
            "{LAUNCHER_ICON_DATA_URL_PREFIX}{}",
            BASE64_STANDARD.encode(bytes)
        )));
    }
    assert!(!valid_icon_data_url("data:image/png;base64,AAAA"));
    assert!(!valid_icon_data_url("data:image/svg+xml;base64,AAAA"));
    let items = full_quota_items();
    validate_launcher_resources(&items).unwrap();
    let mut above = items.clone();
    let old_size =
        (above[0].icon_data_url.as_ref().unwrap().len() - LAUNCHER_ICON_DATA_URL_PREFIX.len()) / 4
            * 3;
    above[0].icon_data_url = Some(format!(
        "{LAUNCHER_ICON_DATA_URL_PREFIX}{}",
        BASE64_STANDARD.encode(padded_png(old_size + 3, MAX_ICON_DIMENSION))
    ));
    assert!(validate_launcher_resources(&above)
        .unwrap_err()
        .contains("合計"));
    assert!(merge_new_launcher_items(&items, vec![item(999)])
        .unwrap_err()
        .contains("200"));
    let existing = &items[..199];
    let mut addition = item(999);
    addition.icon_data_url = Some(at_limit);
    assert!(merge_new_launcher_items(existing, vec![addition])
        .unwrap_err()
        .contains("合計"));
}

#[tokio::test]
async fn duplicate_paths_are_extracted_once_and_oversized_paths_start_no_workers() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Counter(AtomicUsize);
    impl LauncherIconExtractor for Counter {
        fn extract(&self, _: &Path) -> Result<Option<String>, IconExtractionError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }
    }
    let directory =
        std::env::temp_dir().join(format!("rice-launcher-duplicates-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("test.exe");
    std::fs::write(&path, b"fixture").unwrap();
    let extractor = Arc::new(Counter(AtomicUsize::new(0)));
    let config = LauncherWorkerConfig {
        worker_limit: 4,
        worker_pool: Arc::new(Semaphore::new(4)),
        acquire_timeout: Duration::from_secs(1),
        job_timeout: Duration::from_secs(1),
    };
    let result = build_new_items_in_workers_with_extractor(
        vec![],
        vec![path.to_string_lossy().into_owned(); 200],
        extractor.clone(),
        config.clone(),
    )
    .await
    .unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(extractor.0.load(Ordering::SeqCst), 1);
    assert!(build_new_items_in_workers_with_extractor(
        vec![],
        vec![format!("{}.exe", "a".repeat(MAX_PATH_BYTES))],
        extractor.clone(),
        config
    )
    .await
    .is_err());
    assert_eq!(extractor.0.load(Ordering::SeqCst), 1);
    struct InvalidIcon(bool);
    impl LauncherIconExtractor for InvalidIcon {
        fn extract(&self, _: &Path) -> Result<Option<String>, IconExtractionError> {
            if self.0 {
                Err(IconExtractionError::ResourceLimit)
            } else {
                Ok(Some("https://example.com/tracker.png".into()))
            }
        }
    }
    for limit in [true, false] {
        let config = LauncherWorkerConfig {
            worker_limit: 1,
            worker_pool: Arc::new(Semaphore::new(1)),
            acquire_timeout: Duration::from_secs(1),
            job_timeout: Duration::from_secs(1),
        };
        let error = build_new_items_in_workers_with_extractor(
            vec![],
            vec![path.to_string_lossy().into_owned()],
            Arc::new(InvalidIcon(limit)),
            config,
        )
        .await
        .unwrap_err();
        assert!(error.contains("保存していません"));
        assert!(error.contains("小さいアイコン"));
    }
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&directory).unwrap();
}
