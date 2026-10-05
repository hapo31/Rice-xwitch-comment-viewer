use super::*;
use crate::settings::{
    backup_path,
    tests::{cleanup, settings_path_for_test},
    SettingsStore,
};
use std::fs;

fn fixtures() -> Vec<Value> {
    serde_json::from_str(include_str!("../../../tests/fixtures/settings-schema.json")).unwrap()
}
fn expected(fixture: &Value) -> Value {
    let mut settings = serde_json::to_value(AppSettings::default()).unwrap();
    for (section, fields) in fixture["expected"].as_object().unwrap() {
        for (key, value) in fields.as_object().unwrap() {
            settings[section][key] = value.clone();
        }
    }
    settings
}

#[test]
fn versioned_optional_fixtures_preserve_valid_fields_and_default_invalid_leaves() {
    for fixture in fixtures() {
        let decoded = decode(&fixture["input"].to_string()).unwrap();
        assert_eq!(
            serde_json::to_value(&decoded.settings).unwrap(),
            expected(&fixture),
            "{}",
            fixture["name"]
        );
        assert_eq!(
            decoded.read_only,
            fixture["readOnly"].as_bool().unwrap_or(false),
            "{}",
            fixture["name"]
        );
        assert_eq!(
            decoded.unsupported_version,
            fixture["future"].as_bool().unwrap_or(false)
        );
        super::super::validation::validate_settings(&decoded.settings).unwrap();
        crate::launcher::validate_launcher_resources(&decoded.settings.launcher.items).unwrap();
    }
}

#[test]
fn legacy_migration_and_known_fallback_resave_current_schema_with_original_backup() {
    for fixture in fixtures()
        .into_iter()
        .filter(|fixture| !fixture["readOnly"].as_bool().unwrap_or(false))
    {
        let path = settings_path_for_test("schema-migration");
        let original = fixture["input"].to_string();
        fs::write(&path, &original).unwrap();
        let loaded = SettingsStore::load_from_path(&path).unwrap();
        assert_eq!(
            serde_json::to_value(&loaded.settings).unwrap(),
            expected(&fixture),
            "{}",
            fixture["name"]
        );
        assert!(loaded.recovery_notice.is_none());
        if decode(&original).unwrap().needs_resave {
            assert_eq!(fs::read_to_string(backup_path(&path)).unwrap(), original);
        }
        // Explicit save covers optional current documents needing no automatic migration.
        SettingsStore::save_to_path(&path, &loaded.settings).unwrap();
        let saved_text = fs::read_to_string(&path).unwrap();
        let saved: Value = serde_json::from_str(&saved_text).unwrap();
        assert_eq!(saved["schemaVersion"], CURRENT_VERSION);
        let decoded = decode(&saved_text).unwrap();
        assert!(!decoded.read_only && !decoded.needs_resave);
        assert_eq!(
            serde_json::to_value(&decoded.settings).unwrap(),
            expected(&fixture)
        );
        // A supported migration keeps recoverable predecessor bytes; the second
        // normal save rotates it as the existing one-generation backup contract.
        assert!(backup_path(&path).exists());
        let entries: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(entries
            .iter()
            .all(|entry| !entry.file_name().to_string_lossy().contains(".corrupt-")));
        cleanup(&path);
    }
}

#[test]
fn unsupported_documents_never_change_primary_backup_or_create_temporary_files() {
    for fixture in fixtures()
        .into_iter()
        .filter(|fixture| fixture["readOnly"].as_bool().unwrap_or(false))
    {
        let path = settings_path_for_test("schema-read-only");
        SettingsStore::save_to_path(&path, &AppSettings::default()).unwrap();
        SettingsStore::save_to_path(&path, &AppSettings::default()).unwrap();
        let original = fixture["input"].to_string();
        fs::write(&path, &original).unwrap();
        let backup = fs::read(backup_path(&path)).unwrap();
        let listing = || {
            let mut names: Vec<_> = fs::read_dir(path.parent().unwrap())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            names.sort();
            names
        };
        let before = listing();
        let loaded = SettingsStore::load_from_path(&path).unwrap();
        assert_eq!(
            serde_json::to_value(&loaded.settings).unwrap(),
            expected(&fixture)
        );
        if fixture["future"].as_bool().unwrap_or(false) {
            assert!(loaded
                .recovery_notice
                .unwrap()
                .message
                .contains("元ファイル"));
        }
        for fault in [
            super::super::SaveFault::None,
            super::super::SaveFault::TempWrite,
            super::super::SaveFault::Replace,
        ] {
            let error =
                SettingsStore::save_to_path_with_fault(&path, &AppSettings::default(), fault)
                    .unwrap_err();
            assert!(error.is::<ReadOnlySettings>());
            assert_eq!(fs::read_to_string(&path).unwrap(), original);
            assert_eq!(fs::read(backup_path(&path)).unwrap(), backup);
            assert_eq!(listing(), before);
        }
        cleanup(&path);
    }
}

#[test]
fn file_version_changed_after_load_is_rechecked_before_transaction_publish_and_window_save() {
    let path = settings_path_for_test("schema-changed-after-load");
    SettingsStore::save_to_path(&path, &AppSettings::default()).unwrap();
    SettingsStore::save_to_path(&path, &AppSettings::default()).unwrap();
    let mut settings = SettingsStore::load_from_path(&path).unwrap().settings;
    let original_memory = serde_json::to_value(&settings).unwrap();
    let future = "{\"schemaVersion\":999,\"future\":{\"must\":\"preserve exactly\"}}\n";
    let backup = fs::read(backup_path(&path)).unwrap();
    fs::write(&path, future).unwrap();
    let patch =
        serde_json::from_value(serde_json::json!({"twitch":{"channelLogin":"new_channel"}}))
            .unwrap();
    let error = super::super::apply_validated_settings_patch(&mut settings, patch, |candidate| {
        SettingsStore::save_to_path(&path, candidate).map_err(|_| {
            super::super::validation::ValidationError::new(
                "settings",
                "unsupportedSchema",
                READ_ONLY_MESSAGE,
            )
        })
    })
    .unwrap_err();
    assert_eq!(error.code, "unsupportedSchema");
    assert_eq!(serde_json::to_value(&settings).unwrap(), original_memory);
    let mut window_candidate = settings.clone();
    window_candidate.window.position = Some(WindowPosition { x: 99, y: 100 });
    assert!(SettingsStore::save_to_path(&path, &window_candidate).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), future);
    assert_eq!(fs::read(backup_path(&path)).unwrap(), backup);
    cleanup(&path);
}

#[test]
fn malformed_primary_can_restore_a_future_backup_verbatim_and_remain_read_only() {
    let path = settings_path_for_test("schema-future-backup");
    let future = "{\"schemaVersion\":2,\"future\":{\"must\":\"preserve exactly\"}}\n";
    fs::write(&path, "{broken").unwrap();
    fs::write(backup_path(&path), future).unwrap();
    let loaded = SettingsStore::load_from_path(&path).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), future);
    assert_eq!(fs::read_to_string(backup_path(&path)).unwrap(), future);
    assert!(loaded
        .recovery_notice
        .unwrap()
        .message
        .contains("保存しません"));
    assert!(SettingsStore::save_to_path(&path, &loaded.settings)
        .unwrap_err()
        .is::<ReadOnlySettings>());
    cleanup(&path);
}

#[test]
fn arrays_strings_aggregate_rules_and_launcher_quotas_are_bounded_before_domain_publication() {
    for (key, value) in [
        ("blockedUsers", serde_json::json!(vec!["valid_login"; 201])),
        (
            "blockedWords",
            serde_json::json!(vec!["x".repeat(500); 132]),
        ),
        ("blockedWords", serde_json::json!(["😀".repeat(501)])),
        (
            "connectionSuccessSpeechText",
            serde_json::json!("😀".repeat(121)),
        ),
        ("bouyomiHost", serde_json::json!("x".repeat(254))),
    ] {
        let input = serde_json::json!({"speech":{key:value,"autoSpeak":false}});
        let decoded = decode(&input.to_string()).unwrap();
        let mut defaults = AppSettings::default();
        defaults.speech.auto_speak = false;
        assert_eq!(
            serde_json::to_value(&decoded.settings).unwrap(),
            serde_json::to_value(defaults).unwrap(),
            "{key}"
        );
    }
    let items: Vec<_> = (0..201).map(|index| serde_json::json!({"id":format!("item-{index}"),"target":format!("C:\\{index}.exe")})).collect();
    let decoded = decode(&serde_json::json!({"launcher":{"items":items}}).to_string()).unwrap();
    assert!(decoded.settings.launcher.items.is_empty());
    // No unbounded Vec<String>/Value allocation for a many-tiny-node rule array.
    let many_nodes = format!(
        "{{\"speech\":{{\"blockedWords\":[{}]}}}}",
        vec!["\"\""; 100_000].join(",")
    );
    let decoded = decode(&many_nodes).unwrap();
    assert!(decoded.settings.speech.blocked_words.is_empty());
    let users: Vec<_> = (0..200).map(|index| format!("user_{index:019}")).collect();
    let words: Vec<_> = (0..122)
        .map(|index| format!("{index:03}{}", "x".repeat(497)))
        .collect();
    let decoded = decode(&serde_json::json!({"speech":{"blockedUsers":users,"blockedWords":words,"autoSpeak":false}}).to_string()).unwrap();
    assert_eq!(decoded.settings.speech.blocked_users.len(), 200);
    assert!(decoded.settings.speech.blocked_words.is_empty());
    assert!(!decoded.settings.speech.auto_speak);
    let items: Vec<_> = (0..33).map(|index| serde_json::json!({"id":format!("long-{index}"),"target":format!("C:\\{index}{}.exe", "x".repeat(3990))})).collect();
    let decoded = decode(
        &serde_json::json!({"launcher":{"items":items},"speech":{"bouyomiPort":50002}}).to_string(),
    )
    .unwrap();
    assert!(decoded.settings.launcher.items.is_empty());
    assert_eq!(decoded.settings.speech.bouyomi_port, 50002);
}

#[test]
fn duplicate_wire_fields_and_invalid_discriminators_fail_closed_without_losing_data() {
    for input in [
        "{\"schemaVersion\":1,\"schemaVersion\":2}",
        "{\"schemaVersion\":1,\"twitch\":{\"autoConnect\":true,\"autoConnect\":false}}",
        "{\"schemaVersion\":\"future\",\"twitch\":{\"autoConnect\":true}}",
        "{\"schemaVersion\":-1}",
        r#"{"schemaVersion":1,"\ud800":{"preserve":"unknown key"}}"#,
    ] {
        assert!(decode(input).unwrap().read_only, "{input}");
    }
    assert!(decode("{broken").is_err());
    for input in [
        r#"{"schemaVersion":1,"schemaVersion":999,"twitch":{"autoConnect":true,"channelLogin":"future_channel"}}"#,
        r#"{"schemaVersion":999,"schemaVersion":1,"twitch":{"autoConnect":true,"channelLogin":"future_channel"}}"#,
        r#"{"schemaVersion":1,"schemaVersion":1,"twitch":{"autoConnect":true}}"#,
        r#"{"schemaVersion":999,"\ud800":"uninterpretable key","twitch":{"autoConnect":true}}"#,
    ] {
        let decoded = decode(input).unwrap();
        assert!(decoded.read_only && decoded.unsupported_version, "{input}");
        assert!(!decoded.settings.twitch.auto_connect);
        assert!(decoded.settings.twitch.channel_login.is_empty());
        let path = settings_path_for_test("ambiguous-schema-version");
        SettingsStore::save_to_path(&path, &AppSettings::default()).unwrap();
        SettingsStore::save_to_path(&path, &AppSettings::default()).unwrap();
        let backup = fs::read(backup_path(&path)).unwrap();
        fs::write(&path, input).unwrap();
        let loaded = SettingsStore::load_from_path(&path).unwrap();
        assert!(!loaded.settings.twitch.auto_connect);
        assert!(loaded
            .recovery_notice
            .unwrap()
            .message
            .contains("元ファイル"));
        assert!(SettingsStore::save_to_path(&path, &loaded.settings)
            .unwrap_err()
            .is::<ReadOnlySettings>());
        assert_eq!(fs::read_to_string(&path).unwrap(), input);
        assert_eq!(fs::read(backup_path(&path)).unwrap(), backup);
        cleanup(&path);
    }
    let deep_unknown = format!(
        "{{\"schemaVersion\":1,\"future\":{}0{}}}",
        "[".repeat(256),
        "]".repeat(256)
    );
    // If the JSON parser accepts a future shape, never silently save it.
    if let Ok(decoded) = decode(&deep_unknown) {
        assert!(decoded.read_only);
    }
}
