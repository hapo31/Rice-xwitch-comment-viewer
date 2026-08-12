pub mod bouyomi;

#[cfg(feature = "app")]
use crate::app_events::{
    emit_app_log, emit_speech_queue_updated, emit_speech_status, AppLogLevel, SpeechQueueItemEvent,
    SpeechQueueItemStatus, SpeechStatus,
};
#[cfg(feature = "app")]
use crate::settings::AppState;
#[cfg(feature = "app")]
use crate::settings::UrlHandling;
use crate::twitch::{ChatMessage, MessageFragment};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
#[cfg(feature = "app")]
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechRequest {
    pub id: String,
    pub source_message_id: Option<String>,
    pub text: String,
    pub voice: Option<String>,
    pub speed: Option<i16>,
    pub tone: Option<i16>,
    pub volume: Option<i16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeechHealth {
    Connected,
    Disconnected { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeechResult {
    Accepted,
}

#[allow(dead_code)]
pub trait SpeechAdapter: Send + Sync {
    fn health_check(
        &self,
    ) -> impl std::future::Future<Output = anyhow::Result<SpeechHealth>> + Send;
    fn speak(
        &self,
        request: SpeechRequest,
    ) -> impl std::future::Future<Output = anyhow::Result<SpeechResult>> + Send;
    fn pause(&self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
    fn resume(&self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
    fn skip(&self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
    fn clear(&self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
}

const DEFAULT_QUEUE_LIMIT: usize = 200;
// Keep one full queue worth of terminal states. This preserves the status of every
// item after a user clears the maximum-sized pending queue, so the Chat timeline
// cannot continue to show removed items as waiting.
const DEFAULT_HISTORY_LIMIT: usize = DEFAULT_QUEUE_LIMIT;
const DEFAULT_MAX_COMMENT_LENGTH: usize = 120;
const DEFAULT_REPEAT_SUPPRESSION_SECONDS: u64 = 2;
const RETRY_DELAY: Duration = Duration::from_millis(700);

#[derive(Debug)]
pub struct SpeechQueueState {
    pending: VecDeque<SpeechQueueItem>,
    history: VecDeque<SpeechQueueItem>,
    last_user_enqueue: HashMap<String, Instant>,
    next_id: u64,
    is_processing: bool,
    paused: bool,
}

impl Default for SpeechQueueState {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            history: VecDeque::new(),
            last_user_enqueue: HashMap::new(),
            next_id: 1,
            is_processing: false,
            paused: false,
        }
    }
}

#[derive(Debug, Clone)]
struct SpeechQueueItem {
    id: String,
    source_message_id: Option<String>,
    user_display_name: String,
    text: String,
    status: SpeechQueueItemStatus,
    retry_count: u8,
    delivery_state: SpeechQueueDeliveryState,
}

/// `retry_count` は送信を試した回数ではなく、自動再試行を既に予約した回数を表す。
/// 上限に達した項目は pending から履歴へ移すため、通常の worker 再起動では送信対象にならない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpeechQueueDeliveryState {
    Ready,
    RetryScheduled,
    RetryExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpeechQueueFailureTransition {
    RetryScheduled,
    RetryExhausted,
    Ignored,
}

impl SpeechQueueState {
    fn clear_pending(&mut self) {
        while let Some(mut item) = self.pending.pop_front() {
            item.status = SpeechQueueItemStatus::Skipped;
            push_history(self, item);
        }
    }

    fn remove_pending_item(&mut self, item_id: &str) -> bool {
        let Some(index) = self.pending.iter().position(|item| item.id == item_id) else {
            return false;
        };

        let mut item = self.pending.remove(index).expect("queue index checked");
        item.status = SpeechQueueItemStatus::Skipped;
        push_history(self, item);
        true
    }

    fn dismiss_history_item(&mut self, item_id: &str) -> bool {
        let Some(index) = self.history.iter().position(|item| item.id == item_id) else {
            return false;
        };

        self.history.remove(index);
        true
    }

    fn dismiss_history(&mut self) {
        self.history.clear();
    }

    fn has_auto_processable_item(&self) -> bool {
        self.pending
            .front()
            .is_some_and(|item| item.delivery_state == SpeechQueueDeliveryState::Ready)
    }

    fn begin_next_request(&mut self) -> Option<SpeechRequest> {
        let item = self.pending.front_mut()?;
        if item.delivery_state != SpeechQueueDeliveryState::Ready {
            return None;
        }

        item.status = SpeechQueueItemStatus::Speaking;
        Some(SpeechRequest {
            id: item.id.clone(),
            source_message_id: item.source_message_id.clone(),
            text: item.text.clone(),
            voice: None,
            speed: None,
            tone: None,
            volume: None,
        })
    }

    fn complete_request(&mut self, request_id: &str) -> bool {
        let Some(front) = self.pending.front() else {
            return false;
        };
        if front.id != request_id {
            return false;
        }

        let mut item = self.pending.pop_front().expect("front checked");
        item.status = SpeechQueueItemStatus::Spoken;
        push_history(self, item);
        true
    }

    fn fail_request(&mut self, request_id: &str) -> SpeechQueueFailureTransition {
        let Some(front) = self.pending.front() else {
            return SpeechQueueFailureTransition::Ignored;
        };
        if front.id != request_id {
            return SpeechQueueFailureTransition::Ignored;
        }

        if front.retry_count == 0 {
            let item = self.pending.front_mut().expect("front checked");
            item.retry_count = 1;
            item.status = SpeechQueueItemStatus::Queued;
            item.delivery_state = SpeechQueueDeliveryState::RetryScheduled;
            return SpeechQueueFailureTransition::RetryScheduled;
        }

        let mut item = self.pending.pop_front().expect("front checked");
        item.status = SpeechQueueItemStatus::Error;
        item.delivery_state = SpeechQueueDeliveryState::RetryExhausted;
        push_history(self, item);
        SpeechQueueFailureTransition::RetryExhausted
    }

    fn activate_scheduled_retry(&mut self, request_id: &str) -> bool {
        let Some(item) = self.pending.front_mut() else {
            return false;
        };
        if item.id != request_id || item.delivery_state != SpeechQueueDeliveryState::RetryScheduled
        {
            return false;
        }

        item.delivery_state = SpeechQueueDeliveryState::Ready;
        true
    }

    fn retry_exhausted_item(&mut self, item_id: &str) -> bool {
        let Some(index) = self.history.iter().position(|item| {
            item.id == item_id
                && item.status == SpeechQueueItemStatus::Error
                && item.delivery_state == SpeechQueueDeliveryState::RetryExhausted
        }) else {
            return false;
        };

        let mut item = self.history.remove(index).expect("history index checked");
        item.status = SpeechQueueItemStatus::Queued;
        item.retry_count = 0;
        item.delivery_state = SpeechQueueDeliveryState::Ready;
        self.pending.push_back(item);
        true
    }
}

#[derive(Debug, Clone)]
pub struct SpeechFormatter {
    options: SpeechFormatterOptions,
}

#[derive(Debug, Clone)]
pub struct SpeechFormatterOptions {
    pub read_user_name: bool,
    pub max_comment_length: usize,
    pub replace_urls: bool,
    pub block_urls: bool,
    pub escape_bouyomi_tags: bool,
    pub read_emotes: bool,
    pub blocked_users: Vec<String>,
    pub blocked_words: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeechFormatDecision {
    Speak(String),
    Blocked(String),
}

impl Default for SpeechFormatterOptions {
    fn default() -> Self {
        Self {
            read_user_name: true,
            max_comment_length: DEFAULT_MAX_COMMENT_LENGTH,
            replace_urls: true,
            block_urls: false,
            escape_bouyomi_tags: true,
            read_emotes: false,
            blocked_users: Vec::new(),
            blocked_words: Vec::new(),
        }
    }
}

impl SpeechFormatter {
    pub fn new(options: SpeechFormatterOptions) -> Self {
        Self { options }
    }

    pub fn format_chat_message(&self, message: &ChatMessage) -> SpeechFormatDecision {
        let raw_text = collect_readable_text(message, self.options.read_emotes);
        if raw_text.trim().is_empty() {
            return SpeechFormatDecision::Blocked("読み上げる本文がありません。".to_string());
        }

        if contains_blocked_user(&self.options.blocked_users, message) {
            return SpeechFormatDecision::Blocked("NG ユーザーに一致しました。".to_string());
        }

        if self.options.block_urls && contains_url(&raw_text) {
            return SpeechFormatDecision::Blocked("URL を含むため読み上げません。".to_string());
        }

        let lowered = raw_text.to_ascii_lowercase();
        if let Some(blocked_word) = self
            .options
            .blocked_words
            .iter()
            .map(|word| word.trim())
            .filter(|word| !word.is_empty())
            .find(|word| lowered.contains(&word.to_ascii_lowercase()))
        {
            return SpeechFormatDecision::Blocked(format!(
                "NG ワードを含むため読み上げません: {blocked_word}"
            ));
        }

        let mut text = normalize_control_chars(&raw_text);
        if self.options.replace_urls {
            text = replace_urls(&text);
        }
        if self.options.escape_bouyomi_tags {
            text = escape_bouyomi_tags(&text);
        }
        text = collapse_spaces(&text);

        // Normalization and omitted emotes can remove every readable character. Do not
        // turn that into an empty packet or a display-name-only utterance.
        if text.is_empty() {
            return SpeechFormatDecision::Blocked("読み上げる本文がありません。".to_string());
        }

        let max_len = self.options.max_comment_length.max(1);
        if text.chars().count() > max_len {
            text = truncate_chars(&text, max_len);
        }
        if self.options.read_user_name {
            text = format!("{}。{}", message.user_display_name, text);
        }

        SpeechFormatDecision::Speak(text)
    }
}

#[cfg(feature = "app")]
impl From<&crate::settings::SpeechSettings> for SpeechFormatterOptions {
    fn from(settings: &crate::settings::SpeechSettings) -> Self {
        Self {
            read_user_name: settings.read_user_name,
            max_comment_length: settings.max_comment_length as usize,
            replace_urls: matches!(settings.url_handling, UrlHandling::Replace),
            block_urls: matches!(settings.url_handling, UrlHandling::Block),
            escape_bouyomi_tags: true,
            read_emotes: settings.read_emotes,
            blocked_users: settings.blocked_users.clone(),
            blocked_words: settings.blocked_words.clone(),
        }
    }
}

#[cfg(feature = "app")]
pub fn enqueue_chat_message_for_speech(
    app: tauri::AppHandle<tauri::Wry>,
    message: ChatMessage,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (formatter, repeat_suppression_seconds) = {
        let settings = state.settings.lock().map_err(|error| error.to_string())?;
        if !settings.speech.auto_speak {
            return Ok(());
        }
        (
            SpeechFormatter::new(SpeechFormatterOptions::from(&settings.speech)),
            u64::from(settings.speech.repeat_suppression_seconds)
                .max(DEFAULT_REPEAT_SUPPRESSION_SECONDS),
        )
    };

    let mut warning = None;
    let mut should_spawn = false;
    {
        let mut queue = state
            .speech_queue
            .lock()
            .map_err(|error| error.to_string())?;
        let now = Instant::now();
        if let Some(last_seen) = queue.last_user_enqueue.get(&message.user_id) {
            if now.duration_since(*last_seen) < Duration::from_secs(repeat_suppression_seconds) {
                let warning_message =
                    format!("{} の連投を抑制しました。", message.user_display_name);
                let id = next_queue_id(&mut queue);
                push_history(
                    &mut queue,
                    SpeechQueueItem {
                        id,
                        source_message_id: Some(message.id.clone()),
                        user_display_name: message.user_display_name.clone(),
                        text: message.text.clone(),
                        status: SpeechQueueItemStatus::Blocked,
                        retry_count: 0,
                        delivery_state: SpeechQueueDeliveryState::Ready,
                    },
                );
                emit_queue_snapshot(&app, &queue, Some(warning_message));
                return Ok(());
            }
        }

        let formatted_text = match formatter.format_chat_message(&message) {
            SpeechFormatDecision::Speak(text) => text,
            SpeechFormatDecision::Blocked(reason) => {
                let warning_message = format!(
                    "{} のチャットを読み上げません: {reason}",
                    message.user_display_name
                );
                let id = next_queue_id(&mut queue);
                push_history(
                    &mut queue,
                    SpeechQueueItem {
                        id,
                        source_message_id: Some(message.id.clone()),
                        user_display_name: message.user_display_name.clone(),
                        text: message.text.clone(),
                        status: SpeechQueueItemStatus::Blocked,
                        retry_count: 0,
                        delivery_state: SpeechQueueDeliveryState::Ready,
                    },
                );
                emit_queue_snapshot(&app, &queue, Some(warning_message));
                return Ok(());
            }
        };

        queue.last_user_enqueue.insert(message.user_id.clone(), now);
        while queue.pending.len() >= DEFAULT_QUEUE_LIMIT {
            if let Some(mut dropped) = queue.pending.pop_front() {
                dropped.status = SpeechQueueItemStatus::Skipped;
                push_history(&mut queue, dropped);
                warning = Some(
                    "読み上げキューが上限に達したため、古い未読チャットを落としました。"
                        .to_string(),
                );
            }
        }

        let item = SpeechQueueItem {
            id: next_queue_id(&mut queue),
            source_message_id: Some(message.id),
            user_display_name: message.user_display_name,
            text: formatted_text,
            status: SpeechQueueItemStatus::Queued,
            retry_count: 0,
            delivery_state: SpeechQueueDeliveryState::Ready,
        };
        queue.pending.push_back(item);
        if !queue.is_processing && !queue.paused && queue.has_auto_processable_item() {
            queue.is_processing = true;
            should_spawn = true;
        }
        emit_queue_snapshot(&app, &queue, warning);
    }

    if should_spawn {
        tokio::spawn(process_speech_queue(app));
    }
    Ok(())
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
    let state = app.state::<AppState>();
    let mut queue = state
        .speech_queue
        .lock()
        .map_err(|error| error.to_string())?;
    queue.clear_pending();
    emit_queue_snapshot(app, &queue, None);
    Ok(())
}

#[cfg(feature = "app")]
pub fn skip_current_queue_item(app: &tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut should_spawn = false;
    {
        let mut queue = state
            .speech_queue
            .lock()
            .map_err(|error| error.to_string())?;
        if let Some(mut item) = queue.pending.pop_front() {
            item.status = SpeechQueueItemStatus::Skipped;
            push_history(&mut queue, item);
        }
        if !queue.pending.is_empty() && !queue.paused && !queue.is_processing {
            queue.is_processing = true;
            should_spawn = true;
        }
        emit_queue_snapshot(app, &queue, None);
    }
    if should_spawn {
        tokio::spawn(process_speech_queue(app.clone()));
    }
    Ok(())
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
pub fn speech_queue_reload(app: tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    emit_current_queue(&app)
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
        if !queue.retry_exhausted_item(&item_id) {
            return Err(
                "このエラー項目は再試行できません。キューを再読込して状態を確認してください。"
                    .to_string(),
            );
        }
        if !queue.is_processing && !queue.paused && queue.has_auto_processable_item() {
            queue.is_processing = true;
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
    let state = app.state::<AppState>();
    let mut queue = state
        .speech_queue
        .lock()
        .map_err(|error| error.to_string())?;
    queue.paused = true;
    emit_queue_snapshot(app, &queue, None);
    Ok(())
}

#[cfg(feature = "app")]
pub fn resume_queue(app: tauri::AppHandle<tauri::Wry>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut should_spawn = false;
    {
        let mut queue = state
            .speech_queue
            .lock()
            .map_err(|error| error.to_string())?;
        queue.paused = false;
        if !queue.is_processing && queue.has_auto_processable_item() {
            queue.is_processing = true;
            should_spawn = true;
        }
        emit_queue_snapshot(&app, &queue, None);
    }
    if should_spawn {
        tokio::spawn(process_speech_queue(app));
    }
    Ok(())
}

#[cfg(feature = "app")]
async fn process_speech_queue(app: tauri::AppHandle<tauri::Wry>) {
    loop {
        let request = {
            let state = app.state::<AppState>();
            let mut queue = match state.speech_queue.lock() {
                Ok(queue) => queue,
                Err(error) => {
                    emit_app_log(&app, AppLogLevel::Error, error.to_string());
                    return;
                }
            };
            if queue.paused {
                queue.is_processing = false;
                emit_speech_status(
                    &app,
                    SpeechStatus::Paused,
                    Some("読み上げキューを一時停止しました。".to_string()),
                );
                emit_queue_snapshot(&app, &queue, None);
                return;
            }
            if queue.pending.is_empty() {
                queue.is_processing = false;
                emit_speech_status(
                    &app,
                    SpeechStatus::Idle,
                    Some("読み上げキューは空です。".to_string()),
                );
                emit_queue_snapshot(&app, &queue, None);
                return;
            }
            let Some(request) = queue.begin_next_request() else {
                queue.is_processing = false;
                emit_queue_snapshot(&app, &queue, None);
                return;
            };
            emit_queue_snapshot(&app, &queue, None);
            request
        };

        emit_speech_status(
            &app,
            SpeechStatus::Speaking,
            Some("チャットを読み上げています。".to_string()),
        );
        let result = speak_request_from_settings(&app, request.clone()).await;
        match result {
            Ok(()) => {
                let state = app.state::<AppState>();
                let mut queue = match state.speech_queue.lock() {
                    Ok(queue) => queue,
                    Err(error) => {
                        emit_app_log(&app, AppLogLevel::Error, error.to_string());
                        return;
                    }
                };
                queue.complete_request(&request.id);
                emit_queue_snapshot(&app, &queue, None);
            }
            Err(error_message) => {
                let transition;
                {
                    let state = app.state::<AppState>();
                    let mut queue = match state.speech_queue.lock() {
                        Ok(queue) => queue,
                        Err(error) => {
                            emit_app_log(&app, AppLogLevel::Error, error.to_string());
                            return;
                        }
                    };
                    transition = queue.fail_request(&request.id);
                    let queue_message = match transition {
                        SpeechQueueFailureTransition::RetryScheduled => error_message.clone(),
                        SpeechQueueFailureTransition::RetryExhausted => format!(
                            "{error_message} 自動再試行を終了し、エラー履歴へ移しました。Queue の「再試行」で明示的に復旧できます。"
                        ),
                        SpeechQueueFailureTransition::Ignored => error_message.clone(),
                    };
                    emit_speech_status(&app, SpeechStatus::Error, Some(queue_message.clone()));
                    emit_app_log(&app, AppLogLevel::Error, queue_message.clone());
                    emit_queue_snapshot(&app, &queue, Some(queue_message));
                }
                if transition == SpeechQueueFailureTransition::RetryScheduled {
                    tokio::time::sleep(RETRY_DELAY).await;
                    let state = app.state::<AppState>();
                    let mut queue = match state.speech_queue.lock() {
                        Ok(queue) => queue,
                        Err(error) => {
                            emit_app_log(&app, AppLogLevel::Error, error.to_string());
                            return;
                        }
                    };
                    queue.activate_scheduled_retry(&request.id);
                    emit_queue_snapshot(&app, &queue, None);
                }
            }
        }
    }
}

#[cfg(feature = "app")]
async fn speak_request_from_settings(
    app: &tauri::AppHandle<tauri::Wry>,
    request: SpeechRequest,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (host, port, defaults) = {
        let settings = state.settings.lock().map_err(|error| error.to_string())?;
        (
            settings.speech.bouyomi_host.clone(),
            settings.speech.bouyomi_port,
            bouyomi::BouyomiTalkConfig {
                speed: settings.speech.bouyomi_speed,
                tone: settings.speech.bouyomi_tone,
                volume: settings.speech.bouyomi_volume,
                voice: settings.speech.bouyomi_voice,
                code: 0,
            },
        )
    };

    let adapter = bouyomi::BouyomiAdapter::new(&host, port, defaults)?;
    SpeechAdapter::speak(&adapter, request)
        .await
        .map(|_| ())
        .map_err(bouyomi::to_user_message)
}

#[cfg(feature = "app")]
fn emit_queue_snapshot(
    app: &tauri::AppHandle<tauri::Wry>,
    queue: &SpeechQueueState,
    warning: Option<String>,
) {
    let items = queue
        .pending
        .iter()
        .chain(queue.history.iter().rev())
        .take(DEFAULT_QUEUE_LIMIT + DEFAULT_HISTORY_LIMIT)
        .map(to_queue_event_item)
        .collect::<Vec<_>>();
    let queued_count = queue
        .pending
        .iter()
        .filter(|item| {
            matches!(
                item.status,
                SpeechQueueItemStatus::Queued
                    | SpeechQueueItemStatus::Speaking
                    | SpeechQueueItemStatus::Error
            )
        })
        .count();
    emit_speech_queue_updated(app, queued_count, items, warning);
}

#[cfg(feature = "app")]
fn to_queue_event_item(item: &SpeechQueueItem) -> SpeechQueueItemEvent {
    SpeechQueueItemEvent {
        id: item.id.clone(),
        source_message_id: item.source_message_id.clone(),
        user_display_name: item.user_display_name.clone(),
        text: item.text.clone(),
        status: item.status.clone(),
    }
}

fn collect_readable_text(message: &ChatMessage, read_emotes: bool) -> String {
    if message.fragments.is_empty() {
        return message.text.clone();
    }

    message
        .fragments
        .iter()
        .filter_map(|fragment| readable_fragment_text(fragment, read_emotes))
        .collect::<Vec<_>>()
        .join("")
}

fn readable_fragment_text(fragment: &MessageFragment, read_emotes: bool) -> Option<String> {
    if fragment.emote.is_some() && !read_emotes {
        return None;
    }
    Some(fragment.text.clone())
}

fn normalize_control_chars(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_control() || ch == '\n' || ch == '\r' || ch == '\t' {
                ' '
            } else {
                ch
            }
        })
        .collect()
}

fn replace_urls(text: &str) -> String {
    text.split_whitespace()
        .map(|part| {
            let lowered = part.to_ascii_lowercase();
            if lowered.starts_with("http://")
                || lowered.starts_with("https://")
                || lowered.starts_with("www.")
            {
                "URL省略"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_url(text: &str) -> bool {
    text.split_whitespace().any(|part| {
        let lowered = part.to_ascii_lowercase();
        lowered.starts_with("http://")
            || lowered.starts_with("https://")
            || lowered.starts_with("www.")
    })
}

fn contains_blocked_user(blocked_users: &[String], message: &ChatMessage) -> bool {
    blocked_users.iter().any(|user| {
        let user = user.trim().trim_start_matches('@');
        !user.is_empty()
            && (message.user_login.eq_ignore_ascii_case(user)
                || message.user_display_name.eq_ignore_ascii_case(user))
    })
}

fn escape_bouyomi_tags(text: &str) -> String {
    text.replace(')', "）").replace('(', "（")
}

fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, max_len: usize) -> String {
    let mut output = text.chars().take(max_len).collect::<String>();
    output.push('…');
    output
}

#[cfg(feature = "app")]
fn next_queue_id(queue: &mut SpeechQueueState) -> String {
    let id = queue.next_id;
    queue.next_id = queue.next_id.saturating_add(1);
    format!("speech-{id}")
}

fn push_history(queue: &mut SpeechQueueState, item: SpeechQueueItem) {
    queue.history.push_front(item);
    while queue.history.len() > DEFAULT_HISTORY_LIMIT {
        queue.history.pop_back();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twitch::Platform;

    fn queued_item(id: &str) -> SpeechQueueItem {
        SpeechQueueItem {
            id: id.to_string(),
            source_message_id: None,
            user_display_name: "viewer".to_string(),
            text: "こんにちは".to_string(),
            status: SpeechQueueItemStatus::Queued,
            retry_count: 0,
            delivery_state: SpeechQueueDeliveryState::Ready,
        }
    }

    fn history_item(id: &str, status: SpeechQueueItemStatus) -> SpeechQueueItem {
        SpeechQueueItem {
            id: id.to_string(),
            source_message_id: None,
            user_display_name: "viewer".to_string(),
            text: "こんにちは".to_string(),
            status,
            retry_count: 0,
            delivery_state: SpeechQueueDeliveryState::Ready,
        }
    }

    #[test]
    fn removing_a_queued_item_only_cancels_pending_speech() {
        let mut queue = SpeechQueueState::default();
        queue.pending.push_back(queued_item("queued"));
        queue
            .history
            .push_back(history_item("blocked", SpeechQueueItemStatus::Blocked));

        assert!(queue.remove_pending_item("queued"));
        assert!(queue.pending.is_empty());
        assert_eq!(queue.history[0].id, "queued");
        assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Skipped);
        assert_eq!(queue.history[1].id, "blocked");
        assert!(!queue.remove_pending_item("blocked"));
    }

    #[test]
    fn clearing_pending_items_preserves_skipped_history_for_status_consumers() {
        let mut queue = SpeechQueueState::default();
        for index in 0..DEFAULT_QUEUE_LIMIT {
            queue
                .pending
                .push_back(queued_item(&format!("item-{index}")));
        }

        queue.clear_pending();

        assert!(queue.pending.is_empty());
        assert_eq!(queue.history.len(), DEFAULT_QUEUE_LIMIT);
        assert!(queue
            .history
            .iter()
            .all(|item| item.status == SpeechQueueItemStatus::Skipped));
        assert_eq!(queue.history[0].id, "item-199");
        assert_eq!(queue.history[DEFAULT_QUEUE_LIMIT - 1].id, "item-0");
    }

    #[test]
    fn dismissing_history_removes_error_and_blocked_items_without_touching_pending_speech() {
        let mut queue = SpeechQueueState::default();
        queue.pending.push_back(queued_item("queued"));
        queue
            .history
            .push_back(history_item("error", SpeechQueueItemStatus::Error));
        queue
            .history
            .push_back(history_item("blocked", SpeechQueueItemStatus::Blocked));

        assert!(queue.dismiss_history_item("error"));
        assert!(queue.dismiss_history_item("blocked"));
        assert!(queue.history.is_empty());
        assert_eq!(queue.pending[0].id, "queued");
    }

    #[test]
    fn clearing_history_removes_error_and_blocked_items_without_clearing_pending_speech() {
        let mut queue = SpeechQueueState::default();
        queue.pending.push_back(queued_item("queued"));
        queue
            .history
            .push_back(history_item("error", SpeechQueueItemStatus::Error));
        queue
            .history
            .push_back(history_item("blocked", SpeechQueueItemStatus::Blocked));

        queue.dismiss_history();

        assert!(queue.history.is_empty());
        assert_eq!(queue.pending[0].id, "queued");
    }

    fn exhaust_front_item(queue: &mut SpeechQueueState) {
        let initial = queue.begin_next_request().expect("initial request");
        assert_eq!(
            queue.fail_request(&initial.id),
            SpeechQueueFailureTransition::RetryScheduled
        );
        assert!(queue.activate_scheduled_retry(&initial.id));
        let retry = queue.begin_next_request().expect("retry request");
        assert_eq!(
            queue.fail_request(&retry.id),
            SpeechQueueFailureTransition::RetryExhausted
        );
    }

    fn chat(text: &str) -> ChatMessage {
        ChatMessage {
            id: "message-1".to_string(),
            platform: Platform::Twitch,
            channel_id: "channel".to_string(),
            channel_login: "channel".to_string(),
            user_id: "user".to_string(),
            user_login: "viewer".to_string(),
            user_display_name: "viewer".to_string(),
            text: text.to_string(),
            fragments: Vec::new(),
            badges: Vec::new(),
            received_at: "2026-05-23T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn formatter_replaces_urls_and_control_chars() {
        let formatter = SpeechFormatter::new(SpeechFormatterOptions {
            read_user_name: false,
            ..SpeechFormatterOptions::default()
        });

        assert_eq!(
            formatter.format_chat_message(&chat("hello\nhttps://example.com/\u{0007}")),
            SpeechFormatDecision::Speak("hello URL省略".to_string())
        );
    }

    #[test]
    fn formatter_blocks_messages_without_readable_body_after_normalization() {
        let emote_only = ChatMessage {
            fragments: vec![MessageFragment {
                kind: "emote".to_string(),
                text: "Kappa".to_string(),
                emote: Some(crate::twitch::ChatEmote {
                    id: "25".to_string(),
                    emote_set_id: "0".to_string(),
                    owner_id: None,
                }),
                cheermote: None,
            }],
            ..chat("Kappa")
        };
        let cases = [
            ("control chars only", chat("\u{0007}\u{001b}"), None),
            ("line breaks and tabs only", chat("\n\r\t"), None),
            ("emote only", emote_only, None),
            (
                "normal text with control chars",
                chat("hello\u{0007}\n\tworld"),
                Some("hello world"),
            ),
        ];

        for read_user_name in [false, true] {
            let formatter = SpeechFormatter::new(SpeechFormatterOptions {
                read_user_name,
                ..SpeechFormatterOptions::default()
            });

            for (case_name, message, expected_text) in &cases {
                match expected_text {
                    Some(expected_text) => {
                        let expected = if read_user_name {
                            format!("viewer。{expected_text}")
                        } else {
                            expected_text.to_string()
                        };
                        assert_eq!(
                            formatter.format_chat_message(message),
                            SpeechFormatDecision::Speak(expected),
                            "{case_name}"
                        );
                    }
                    None => assert_eq!(
                        formatter.format_chat_message(message),
                        SpeechFormatDecision::Blocked("読み上げる本文がありません。".to_string()),
                        "{case_name}, read_user_name={read_user_name}"
                    ),
                }
            }
        }
    }

    #[test]
    fn formatter_truncates_long_chat_messages() {
        let formatter = SpeechFormatter::new(SpeechFormatterOptions {
            read_user_name: false,
            max_comment_length: 5,
            ..SpeechFormatterOptions::default()
        });

        assert_eq!(
            formatter.format_chat_message(&chat("123456789")),
            SpeechFormatDecision::Speak("12345…".to_string())
        );
    }

    #[test]
    fn formatter_blocks_ng_words() {
        let formatter = SpeechFormatter::new(SpeechFormatterOptions {
            blocked_words: vec!["badword".to_string()],
            ..SpeechFormatterOptions::default()
        });

        assert!(matches!(
            formatter.format_chat_message(&chat("this has BADWORD")),
            SpeechFormatDecision::Blocked(_)
        ));
    }

    #[test]
    fn formatter_blocks_ng_users() {
        let formatter = SpeechFormatter::new(SpeechFormatterOptions {
            blocked_users: vec!["viewer".to_string()],
            ..SpeechFormatterOptions::default()
        });

        assert!(matches!(
            formatter.format_chat_message(&chat("hello")),
            SpeechFormatDecision::Blocked(_)
        ));
    }

    #[test]
    fn formatter_prepends_display_name_without_honorific() {
        let formatter = SpeechFormatter::new(SpeechFormatterOptions {
            read_user_name: true,
            ..SpeechFormatterOptions::default()
        });

        assert_eq!(
            formatter.format_chat_message(&chat("こんにちは")),
            SpeechFormatDecision::Speak("viewer。こんにちは".to_string())
        );
    }

    #[test]
    fn formatter_blocks_urls_when_configured() {
        let formatter = SpeechFormatter::new(SpeechFormatterOptions {
            block_urls: true,
            ..SpeechFormatterOptions::default()
        });

        assert!(matches!(
            formatter.format_chat_message(&chat("https://example.com")),
            SpeechFormatDecision::Blocked(_)
        ));
    }

    #[test]
    fn formatter_escapes_bouyomi_tags() {
        let formatter = SpeechFormatter::new(SpeechFormatterOptions {
            read_user_name: false,
            ..SpeechFormatterOptions::default()
        });

        assert_eq!(
            formatter.format_chat_message(&chat("(speed 300) test")),
            SpeechFormatDecision::Speak("（speed 300） test".to_string())
        );
    }

    #[test]
    fn first_failure_schedules_exactly_one_automatic_retry() {
        let mut queue = SpeechQueueState::default();
        queue.pending.push_back(queued_item("speech-1"));

        let request = queue.begin_next_request().expect("initial request");
        assert_eq!(
            queue.fail_request(&request.id),
            SpeechQueueFailureTransition::RetryScheduled
        );
        let item = queue.pending.front().expect("scheduled item");
        assert_eq!(item.retry_count, 1);
        assert_eq!(item.status, SpeechQueueItemStatus::Queued);
        assert_eq!(
            item.delivery_state,
            SpeechQueueDeliveryState::RetryScheduled
        );
        assert!(queue.begin_next_request().is_none());

        assert!(queue.activate_scheduled_retry(&request.id));
        assert_eq!(
            queue.begin_next_request().expect("retry request").id,
            "speech-1"
        );
    }

    #[test]
    fn successful_retry_moves_item_to_spoken_history() {
        let mut queue = SpeechQueueState::default();
        queue.pending.push_back(queued_item("speech-1"));

        let initial = queue.begin_next_request().expect("initial request");
        assert_eq!(
            queue.fail_request(&initial.id),
            SpeechQueueFailureTransition::RetryScheduled
        );
        assert!(queue.activate_scheduled_retry(&initial.id));
        let retry = queue.begin_next_request().expect("retry request");
        assert!(queue.complete_request(&retry.id));

        assert!(queue.pending.is_empty());
        let item = queue.history.front().expect("spoken history");
        assert_eq!(item.status, SpeechQueueItemStatus::Spoken);
        assert_eq!(item.retry_count, 1);
    }

    #[test]
    fn exhausted_item_isolated_and_does_not_block_later_items() {
        let mut queue = SpeechQueueState::default();
        queue.pending.push_back(queued_item("speech-a"));
        queue.pending.push_back(queued_item("speech-b"));

        exhaust_front_item(&mut queue);

        let failed = queue.history.front().expect("failed history");
        assert_eq!(failed.id, "speech-a");
        assert_eq!(failed.status, SpeechQueueItemStatus::Error);
        assert_eq!(failed.retry_count, 1);
        assert_eq!(
            failed.delivery_state,
            SpeechQueueDeliveryState::RetryExhausted
        );
        assert_eq!(
            queue.begin_next_request().expect("next item request").id,
            "speech-b"
        );
    }

    #[test]
    fn enqueue_and_resume_do_not_revive_an_exhausted_item() {
        let mut queue = SpeechQueueState::default();
        queue.pending.push_back(queued_item("speech-a"));
        exhaust_front_item(&mut queue);

        assert!(!queue.has_auto_processable_item());
        queue.pending.push_back(queued_item("speech-b"));
        assert!(queue.has_auto_processable_item());
        assert_eq!(
            queue.begin_next_request().expect("new item request").id,
            "speech-b"
        );
        let failed = queue.history.front().expect("failed history");
        assert_eq!(failed.id, "speech-a");
        assert_eq!(failed.retry_count, 1);
        assert_eq!(
            failed.delivery_state,
            SpeechQueueDeliveryState::RetryExhausted
        );
    }

    #[test]
    fn manual_retry_explicitly_restores_the_automatic_retry_budget() {
        let mut queue = SpeechQueueState::default();
        queue.pending.push_back(queued_item("speech-1"));
        exhaust_front_item(&mut queue);

        assert!(queue.retry_exhausted_item("speech-1"));
        assert!(queue.history.is_empty());
        let item = queue.pending.front().expect("manually requeued item");
        assert_eq!(item.status, SpeechQueueItemStatus::Queued);
        assert_eq!(item.retry_count, 0);
        assert_eq!(item.delivery_state, SpeechQueueDeliveryState::Ready);

        let request = queue.begin_next_request().expect("manual retry request");
        assert_eq!(
            queue.fail_request(&request.id),
            SpeechQueueFailureTransition::RetryScheduled
        );
    }
}
