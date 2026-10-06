#[test]
fn backend_layers_keep_settings_and_speech_dependencies_inward() {
    let settings_model = include_str!("settings/model.rs");
    let settings_validation = include_str!("settings/validation.rs");
    let settings_storage = include_str!("settings/persistence.rs");
    let settings_facade = include_str!("settings/mod.rs");
    let app_state = include_str!("application.rs");
    let speech_endpoint = include_str!("speech/endpoint.rs");
    let speech_runtime = include_str!("speech/runtime.rs");
    let speech_failure = include_str!("speech/failure.rs");
    let speech_types = include_str!("speech/types.rs");
    let app_events = include_str!("app_events/mod.rs");
    let speech_module = include_str!("speech/mod.rs");
    let speech_events = include_str!("speech/events.rs");
    let speech_worker = include_str!("speech/worker.rs");
    let speech_domain_types = include_str!("speech/types.rs");

    assert!(!settings_model.contains("tauri::"));
    assert!(!settings_model.contains("settings::persistence"));
    assert!(!settings_model.contains("speech::bouyomi::"));
    assert!(!settings_validation.contains("speech::bouyomi::"));
    assert!(settings_validation.contains("speech::endpoint::"));
    assert!(!settings_storage.contains("struct AppState"));
    assert!(!settings_facade.contains("pub struct AppState"));
    assert!(app_state.contains("pub struct AppState"));

    assert!(!speech_endpoint.contains("super::bouyomi"));
    assert!(!speech_runtime.contains("AppState"));
    assert!(!speech_failure.contains("app_events"));
    assert!(speech_types.contains("pub enum SpeechStatus"));
    assert!(app_events.contains("pub use crate::speech::{"));
    assert!(speech_domain_types.contains("pub(crate) enum SpeechLogLevel"));
    assert!(speech_worker.contains("SpeechLogLevel"));
    assert!(!speech_worker.contains("AppLogLevel"));
    assert!(!speech_worker.contains("app_events::"));
    assert!(!speech_module.contains("SpeechQueueUpdatedEvent {"));
    assert!(!speech_module.contains("SpeechQueueItemEvent {"));
    assert!(!speech_events.contains("SpeechQueueUpdatedEvent {"));
    assert!(!speech_events.contains("SpeechQueueItemEvent {"));
    assert!(speech_events.contains("crate::app_events::speech_queue_updated_event("));
    assert!(app_events.contains("pub(crate) fn speech_queue_updated_event("));
}
