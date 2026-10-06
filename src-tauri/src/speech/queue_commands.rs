use super::events::{emit_queue_snapshot, TauriSpeechQueueEvents};
use super::formatter::{SpeechFormatter, SpeechFormatterOptions};
use super::queue::{enqueue_message, REPEAT_SUPPRESSION_CLEANUP_INTERVAL};
#[cfg(feature = "app")]
use super::worker;
use super::SpeechControl;
use crate::app_events::{AppEventState, SpeechStateSnapshot};
use crate::settings::AppState;
use crate::twitch::ChatMessage;
use tauri::Manager;

#[cfg(feature = "app")]
pub fn enqueue_chat_message_for_speech(
    app: tauri::AppHandle<tauri::Wry>,
    message: ChatMessage,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (formatter, speech_settings) = {
        let settings = state.settings.lock().map_err(|error| error.to_string())?;
        (
            SpeechFormatter::new(SpeechFormatterOptions::from(&settings.speech)),
            settings.speech.clone(),
        )
    };
    let outcome = {
        let mut queue = state
            .speech_queue
            .lock()
            .map_err(|error| error.to_string())?;
        let outcome = enqueue_message(
            &mut queue,
            &speech_settings,
            &formatter,
            message,
            state.speech_runtime.clock.now(),
        );
        emit_queue_snapshot(&app, &queue, outcome.warning.clone());
        outcome
    };
    if outcome.should_spawn {
        tokio::spawn(process_speech_queue(app.clone()));
    }
    if outcome.should_schedule_cleanup {
        tokio::spawn(process_repeat_suppression_cleanup(app));
    }
    Ok(())
}

#[cfg(feature = "app")]
async fn process_repeat_suppression_cleanup(app: tauri::AppHandle<tauri::Wry>) {
    loop {
        let cleanup_is_due = {
            let state = app.state::<AppState>();
            let Ok(mut queue) = state.speech_queue.lock() else {
                return;
            };
            queue.run_repeat_suppression_cleanup_turn(state.speech_runtime.clock.now())
        };

        let Some(cleanup_is_due) = cleanup_is_due else {
            return;
        };

        // When more than one cleanup batch is already expired, release the mutex
        // between batches instead of waiting another second for each one.
        if cleanup_is_due {
            tokio::task::yield_now().await;
        } else {
            app.state::<AppState>()
                .speech_runtime
                .clock
                .sleep(REPEAT_SUPPRESSION_CLEANUP_INTERVAL)
                .await;
        }
    }
}

#[cfg(feature = "app")]
pub fn emit_current_queue(app: &tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let queue = state
        .speech_queue
        .lock()
        .map_err(|error| error.to_string())?;
    emit_queue_snapshot(app, &queue, None);
    Ok(())
}

#[cfg(feature = "app")]
pub fn clear_speech_queue(app: &tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    apply_queue_control(app, SpeechControl::Clear)
}

#[cfg(feature = "app")]
pub fn skip_current_queue_item(app: &tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    apply_queue_control(app, SpeechControl::Skip)
}

#[cfg(feature = "app")]
pub fn remove_queue_item(app: &tauri::AppHandle<tauri::Wry>, item_id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut queue = state
        .speech_queue
        .lock()
        .map_err(|error| error.to_string())?;
    queue.remove_pending_item(item_id);
    emit_queue_snapshot(app, &queue, None);
    Ok(())
}

#[cfg(feature = "app")]
pub fn dismiss_queue_history_item(
    app: &tauri::AppHandle<tauri::Wry>,
    item_id: &str,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut queue = state
        .speech_queue
        .lock()
        .map_err(|error| error.to_string())?;
    queue.dismiss_history_item(item_id);
    emit_queue_snapshot(app, &queue, None);
    Ok(())
}

#[cfg(feature = "app")]
pub fn dismiss_queue_history(app: &tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut queue = state
        .speech_queue
        .lock()
        .map_err(|error| error.to_string())?;
    queue.dismiss_history();
    emit_queue_snapshot(app, &queue, None);
    Ok(())
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn speech_queue_reload(
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<SpeechStateSnapshot, String> {
    let state = app.state::<AppState>();
    // Keep the queue lock while reading the event state so a queue event cannot
    // land between the payload read and the revision captured for the snapshot.
    let queue = state
        .speech_queue
        .lock()
        .map_err(|error| error.to_string())?;
    let _queue_guard = queue;
    let event_state = app
        .try_state::<AppEventState>()
        .ok_or_else(|| "アプリ状態を再読込できません。".to_string())?;
    event_state
        .speech_state_snapshot()
        .ok_or_else(|| "読み上げ状態をまだ取得できません。".to_string())
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn speech_queue_remove(
    app: tauri::AppHandle<tauri::Wry>,
    item_id: String,
) -> Result<(), String> {
    remove_queue_item(&app, &item_id)
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn speech_queue_dismiss(
    app: tauri::AppHandle<tauri::Wry>,
    item_id: String,
) -> Result<(), String> {
    dismiss_queue_history_item(&app, &item_id)
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn speech_queue_dismiss_history(app: tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    dismiss_queue_history(&app)
}

#[cfg(feature = "app")]
#[tauri::command]
pub fn speech_queue_retry(
    app: tauri::AppHandle<tauri::Wry>,
    item_id: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut should_spawn = false;
    {
        let mut queue = state
            .speech_queue
            .lock()
            .map_err(|error| error.to_string())?;
        if !queue.has_retry_capacity() {
            return Err("読み上げキューは上限の200件です。待機項目が減ってから再試行してください。エラー履歴はそのまま保持しています。".to_string());
        }
        if !queue.retry_exhausted_item(&item_id) {
            return Err(
                "このエラー項目は再試行できません。キューを再読込して状態を確認してください。"
                    .to_string(),
            );
        }
        if queue.claim_worker() {
            should_spawn = true;
        }
        emit_queue_snapshot(
            &app,
            &queue,
            Some(
                "エラー項目をキューの末尾へ戻しました。自動再試行は再度 1 回までです。".to_string(),
            ),
        );
    }
    if should_spawn {
        tokio::spawn(process_speech_queue(app));
    }
    Ok(())
}

#[cfg(feature = "app")]
pub fn pause_queue(app: &tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    apply_queue_control(app, SpeechControl::Pause)
}

#[cfg(feature = "app")]
pub fn resume_queue(app: &tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    apply_queue_control(app, SpeechControl::Resume)
}

#[cfg(feature = "app")]
fn apply_queue_control(
    app: &tauri::AppHandle<tauri::Wry>,
    command: SpeechControl,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let should_spawn = {
        let mut queue = state
            .speech_queue
            .lock()
            .map_err(|error| error.to_string())?;
        let should_spawn = queue.apply_control(command);
        emit_queue_snapshot(app, &queue, None);
        should_spawn
    };
    if should_spawn {
        tokio::spawn(process_speech_queue(app.clone()));
    }
    Ok(())
}

#[cfg(feature = "app")]
async fn process_speech_queue(app: tauri::AppHandle<tauri::Wry>) {
    let state = app.state::<AppState>();
    let selector_app = app.clone();
    let worker = worker::SpeechQueueWorker {
        queue: state.speech_queue.clone(),
        dispatcher: state.speech_runtime.dispatcher(),
        clock: state.speech_runtime.clock.clone(),
        select: std::sync::Arc::new(move || {
            let state = selector_app.state::<AppState>();
            state.speech_runtime.select_from_state(&state)
        }),
        events: std::sync::Arc::new(TauriSpeechQueueEvents(app.clone())),
    };
    worker.run().await;
}

#[cfg(feature = "app")]
pub(crate) fn begin_queue_control(app: &tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut queue = state
        .speech_queue
        .lock()
        .map_err(|error| error.to_string())?;
    queue.begin_control();
    Ok(())
}

#[cfg(feature = "app")]
pub(crate) fn cancel_queue_control(app: &tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let should_spawn = {
        let mut queue = state
            .speech_queue
            .lock()
            .map_err(|error| error.to_string())?;
        queue.cancel_control_and_claim_worker()
    };
    if should_spawn {
        tokio::spawn(process_speech_queue(app.clone()));
    }
    Ok(())
}
