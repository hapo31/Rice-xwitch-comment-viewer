use super::{SpeechControl, SpeechFailure, SpeechHealth, SpeechRequest};
use crate::app_events::{
    emit_app_log, emit_speech_adapter_health, emit_speech_status, AppLogLevel, SpeechAdapterHealth,
    SpeechStatus,
};
use crate::settings::AppState;

fn selected(state: &AppState) -> Result<super::runtime::SelectedSpeechAdapter, SpeechFailure> {
    state.speech_runtime.select_from_state(state)
}

async fn checked_health(session: &super::runtime::SpeechSession) -> Result<(), SpeechFailure> {
    match session.health_check().await? {
        SpeechHealth::Connected => Ok(()),
        SpeechHealth::Disconnected { failure } => Err(failure),
    }
}

#[tauri::command]
pub async fn speech_health_check(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<String, String> {
    let adapter = selected(&state).map_err(|failure| report_failure(&app, failure))?;
    let started = state.speech_runtime.clock.now();
    let session = adapter.lock().await;
    checked_health(&session)
        .await
        .map_err(|failure| report_failure(&app, failure))?;
    if let Some(text) = &adapter.confirmation_text {
        session
            .speak(SpeechRequest {
                id: "connection-confirmation".to_string(),
                source_message_id: None,
                text: text.clone(),
            })
            .await
            .map_err(|failure| report_failure(&app, failure))?;
    }
    let message = format!(
        "読み上げ先に接続できました。応答時間 {}ms",
        state
            .speech_runtime
            .clock
            .now()
            .saturating_duration_since(started)
            .as_millis()
    );
    emit_speech_adapter_health(&app, SpeechAdapterHealth::Connected, Some(message.clone()));
    emit_app_log(&app, AppLogLevel::Info, message.clone());
    Ok(message)
}

#[tauri::command]
pub async fn speech_health_probe(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<String, String> {
    let adapter = selected(&state).map_err(|failure| report_failure(&app, failure))?;
    let started = state.speech_runtime.clock.now();
    let session = adapter.lock().await;
    checked_health(&session)
        .await
        .map_err(|failure| report_failure(&app, failure))?;
    emit_speech_adapter_health(
        &app,
        SpeechAdapterHealth::Connected,
        Some("読み上げ先の接続を確認しました。".to_string()),
    );
    Ok(format!(
        "読み上げ先に接続できました。応答時間 {}ms",
        state
            .speech_runtime
            .clock
            .now()
            .saturating_duration_since(started)
            .as_millis()
    ))
}

#[tauri::command]
pub async fn speech_test(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
    text: String,
) -> Result<(), String> {
    let adapter = selected(&state).map_err(|failure| report_failure(&app, failure))?;
    let text = normalize_test_text(&text);
    emit_speech_status(
        &app,
        SpeechStatus::Speaking,
        Some("テスト読み上げを送信しています。".to_string()),
    );
    let session = adapter.lock().await;
    session
        .speak(SpeechRequest {
            id: "speech-test".to_string(),
            source_message_id: None,
            text,
        })
        .await
        .map_err(|failure| report_failure(&app, failure))?;
    emit_speech_status(
        &app,
        SpeechStatus::Idle,
        Some("テスト読み上げを送信しました。".to_string()),
    );
    emit_app_log(&app, AppLogLevel::Info, "テスト読み上げを送信しました。");
    Ok(())
}

#[tauri::command]
pub async fn speech_pause(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    control_from_settings(&state, &app, SpeechControl::Pause, |app| {
        super::pause_queue(app)?;
        emit_speech_status(
            app,
            SpeechStatus::Paused,
            Some("読み上げを一時停止しました。".to_string()),
        );
        emit_app_log(app, AppLogLevel::Info, "読み上げを一時停止しました。");
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn speech_resume(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    control_from_settings(&state, &app, SpeechControl::Resume, |app| {
        super::resume_queue(app)?;
        emit_speech_status(
            app,
            SpeechStatus::Idle,
            Some("読み上げを再開しました。".to_string()),
        );
        emit_app_log(app, AppLogLevel::Info, "読み上げを再開しました。");
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn speech_skip(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    control_from_settings(&state, &app, SpeechControl::Skip, |app| {
        super::skip_current_queue_item(app)?;
        emit_speech_status(
            app,
            SpeechStatus::Idle,
            Some("現在の読み上げをスキップしました。".to_string()),
        );
        emit_app_log(app, AppLogLevel::Info, "現在の読み上げをスキップしました。");
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn speech_clear(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    control_from_settings(&state, &app, SpeechControl::Clear, |app| {
        super::clear_speech_queue(app)?;
        emit_speech_status(
            app,
            SpeechStatus::Idle,
            Some("読み上げキューをクリアしました。".to_string()),
        );
        emit_app_log(app, AppLogLevel::Info, "読み上げキューをクリアしました。");
        Ok(())
    })
    .await
}

pub(crate) fn report_failure(app: &tauri::AppHandle<tauri::Wry>, failure: SpeechFailure) -> String {
    emit_speech_adapter_health(
        app,
        failure.adapter_health(),
        Some(failure.user_message.clone()),
    );
    let level = if failure.status == SpeechStatus::Disconnected {
        AppLogLevel::Warning
    } else {
        AppLogLevel::Error
    };
    emit_app_log(app, level, failure.log_message());
    failure.user_message
}

async fn control_from_settings(
    state: &AppState,
    app: &tauri::AppHandle<tauri::Wry>,
    command: SpeechControl,
    apply_local: fn(&tauri::AppHandle<tauri::Wry>) -> Result<(), String>,
) -> Result<(), String> {
    super::begin_queue_control(app)
        .map_err(|error| report_failure(app, SpeechFailure::unknown(error)))?;
    let adapter = match selected(state) {
        Ok(adapter) => adapter,
        Err(mut failure) => {
            let _ = super::cancel_queue_control(app);
            failure.user_message = format!(
                "制御は送信しておらず、キューは変更していません。 {}",
                failure.user_message
            );
            return Err(report_failure(app, failure));
        }
    };
    let session = adapter.lock().await;
    match session.control(command).await {
        Ok(()) => match apply_local(app) {
            Ok(()) => Ok(()),
            Err(error) => {
                let mut failure = SpeechFailure::unknown(error);
                failure.user_message = "制御は送信済みですが、アプリ内のキューへ反映できませんでした。読み上げ先とキューの状態、Logsの詳細を確認してください。".to_string();
                Err(report_failure(app, failure))
            }
        },
        Err(mut failure) => {
            let _ = super::cancel_queue_control(app);
            failure.user_message = control_failure_message(command, &failure.user_message);
            Err(report_failure(app, failure))
        }
    }
}

fn control_failure_message(command: SpeechControl, error: &str) -> String {
    let operation = match command {
        SpeechControl::Pause => "一時停止",
        SpeechControl::Resume => "再開",
        SpeechControl::Skip => "スキップ",
        SpeechControl::Clear => "クリア",
    };
    format!("読み上げ先へ{operation}を送信できなかったため、アプリ内の読み上げキューは変更していません。相手側には届いている可能性があるため、状態を確認してください: {error}")
}

fn normalize_test_text(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        "テスト読み上げです。".to_string()
    } else {
        text.chars().take(120).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_failure_keeps_local_and_remote_state_distinguishable() {
        let message = control_failure_message(SpeechControl::Clear, "write timed out");
        assert!(message.contains("アプリ内の読み上げキューは変更していません"));
        assert!(message.contains("相手側には届いている可能性"));
        assert!(message.contains("write timed out"));
    }
}
