#[cfg(any(feature = "app", test))]
use super::formatter::{SpeechFormatDecision, SpeechFormatter};
use super::outcome::{self, BlockedReason, SkippedReason, SpeechQueueOutcome};
#[cfg(test)]
use super::FailureCode;
#[cfg(any(feature = "app", test))]
use super::SpeechQueuePhase;
use super::{SpeechControl, SpeechFailure, SpeechRequest};
use crate::app_events::SpeechQueueItemStatus;
#[cfg(test)]
use crate::app_events::SpeechStatus;
use crate::settings::SpeechSettings;
use crate::twitch::ChatMessage;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

pub(crate) const DEFAULT_QUEUE_LIMIT: usize = 200;
// Keep one full queue worth of terminal states. This preserves the status of every
// item after a user clears the maximum-sized pending queue, so the Chat timeline
// cannot continue to show removed items as waiting.
pub(crate) const DEFAULT_HISTORY_LIMIT: usize = DEFAULT_QUEUE_LIMIT;
pub(crate) const RETRY_DELAY: Duration = Duration::from_millis(700);
// Twitch の設定値は 30 秒までで、background cleanup は通常1秒以内に期限を観測する。
// 実際の解放時刻は runtime のスケジューリングと mutex 待ちの影響を受ける。
// cleanup は background task と enqueue の両方で固定件数だけ進め、休止後の大量コメントで
// queue mutex を長時間保持しない。期限キューが map entry を所有するため、両方の保持量を
// 同じ上限に固定できる。
pub(super) const MAX_REPEAT_SUPPRESSION_WINDOW: Duration = Duration::from_secs(30);
pub(super) const MAX_REPEAT_SUPPRESSION_ENTRIES: usize = 4_096;
pub(super) const REPEAT_SUPPRESSION_CLEANUP_BATCH: usize = 64;
#[cfg(feature = "app")]
pub(super) const REPEAT_SUPPRESSION_CLEANUP_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepeatSuppressionScope {
    pub(super) channel_id: String,
    pub(super) connection_generation: Option<u64>,
}

#[derive(Debug)]
pub(super) struct RepeatSuppressionExpiry {
    pub(super) expires_at: Instant,
    pub(super) user_id: String,
    pub(super) accepted_at: Instant,
}

#[derive(Debug)]
pub struct SpeechQueueState {
    pub(super) pending: VecDeque<SpeechQueueItem>,
    pub(super) in_flight: Option<SpeechQueueItem>,
    pub(super) history: VecDeque<SpeechQueueItem>,
    pub(super) last_user_enqueue: HashMap<String, Instant>,
    pub(super) repeat_suppression_expirations: VecDeque<RepeatSuppressionExpiry>,
    pub(super) repeat_suppression_scope: Option<RepeatSuppressionScope>,
    pub(super) repeat_suppression_cleanup_scheduled: bool,
    pub(super) next_id: u64,
    pub(super) is_processing: bool,
    pub(super) paused: bool,
    pub(super) controls_in_progress: usize,
}

impl Default for SpeechQueueState {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            in_flight: None,
            history: VecDeque::new(),
            last_user_enqueue: HashMap::new(),
            repeat_suppression_expirations: VecDeque::new(),
            repeat_suppression_scope: None,
            repeat_suppression_cleanup_scheduled: false,
            next_id: 1,
            is_processing: false,
            paused: false,
            controls_in_progress: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SpeechQueueItem {
    pub(super) id: String,
    pub(super) source_message_id: Option<String>,
    pub(super) user_display_name: String,
    pub(super) text: String,
    pub(super) status: SpeechQueueItemStatus,
    pub(super) retry_count: u8,
    pub(super) delivery_state: SpeechQueueDeliveryState,
    pub(super) outcome: Option<SpeechQueueOutcome>,
}

/// `retry_count` は送信を試した回数ではなく、自動再試行を既に予約した回数を表す。
/// 上限に達した項目は pending から履歴へ移すため、通常の worker 再起動では送信対象にならない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpeechQueueDeliveryState {
    Ready,
    RetryScheduled,
    RetryExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpeechQueueFailureTransition {
    RetryScheduled,
    RetryExhausted,
    Ignored,
}

impl SpeechQueueState {
    #[cfg(any(feature = "app", test))]
    pub(crate) fn snapshot(&self, warning: Option<String>) -> super::SpeechQueueSnapshot {
        let items = self
            .in_flight
            .iter()
            .chain(self.pending.iter())
            .chain(self.history.iter().rev())
            .take(DEFAULT_QUEUE_LIMIT + DEFAULT_HISTORY_LIMIT)
            .map(|item| super::SpeechQueueItemSnapshot {
                id: item.id.clone(),
                source_message_id: item.source_message_id.clone(),
                user_display_name: item.user_display_name.clone(),
                text: item.text.clone(),
                status: item.status.clone(),
                outcome: item.outcome.clone(),
            })
            .collect();
        let queued_count = self
            .in_flight
            .iter()
            .chain(self.pending.iter())
            .filter(|item| {
                matches!(
                    item.status,
                    SpeechQueueItemStatus::Queued
                        | SpeechQueueItemStatus::Speaking
                        | SpeechQueueItemStatus::Error
                )
            })
            .count();
        let phase = if self.paused {
            SpeechQueuePhase::Paused
        } else if self.in_flight.is_some() {
            SpeechQueuePhase::Speaking
        } else if self.pending.is_empty()
            && self
                .history
                .iter()
                .any(|item| item.status == SpeechQueueItemStatus::Error)
        {
            SpeechQueuePhase::Error
        } else {
            SpeechQueuePhase::Idle
        };
        super::SpeechQueueSnapshot {
            queued_count,
            items,
            phase,
            warning,
        }
    }

    pub(super) fn begin_control(&mut self) {
        self.controls_in_progress = self.controls_in_progress.saturating_add(1);
    }

    pub(super) fn apply_control(&mut self, command: SpeechControl) -> bool {
        self.controls_in_progress = self.controls_in_progress.saturating_sub(1);
        match command {
            SpeechControl::Pause => self.paused = true,
            SpeechControl::Resume => self.paused = false,
            SpeechControl::Skip => self.skip_current(),
            SpeechControl::Clear => self.clear_pending(),
        }
        matches!(command, SpeechControl::Resume | SpeechControl::Skip) && self.claim_worker()
    }
    pub(super) fn prepare_repeat_suppression(
        &mut self,
        message: &ChatMessage,
        repeat_suppression_seconds: u16,
        now: Instant,
    ) {
        let scope = RepeatSuppressionScope {
            channel_id: message.channel_id.clone(),
            connection_generation: message.connection_generation,
        };
        if self.repeat_suppression_scope.as_ref() != Some(&scope) {
            self.clear_repeat_suppression_entries();
            self.repeat_suppression_scope = Some(scope);
        }

        if repeat_suppression_seconds == 0 {
            self.clear_repeat_suppression_entries();
            return;
        }

        self.cleanup_expired_repeat_suppression_entries(now);
    }

    pub(super) fn record_user_enqueue(&mut self, user_id: String, now: Instant) {
        if self.repeat_suppression_expirations.len() == MAX_REPEAT_SUPPRESSION_ENTRIES {
            self.remove_repeat_suppression_expiry();
        }

        self.last_user_enqueue.insert(user_id.clone(), now);
        self.repeat_suppression_expirations
            .push_back(RepeatSuppressionExpiry {
                expires_at: now + MAX_REPEAT_SUPPRESSION_WINDOW,
                user_id,
                accepted_at: now,
            });
    }

    pub(super) fn clear_repeat_suppression_entries(&mut self) {
        self.last_user_enqueue.clear();
        self.repeat_suppression_expirations.clear();
    }

    pub(super) fn cleanup_expired_repeat_suppression_entries(&mut self, now: Instant) {
        for _ in 0..REPEAT_SUPPRESSION_CLEANUP_BATCH {
            let Some(expiry) = self.repeat_suppression_expirations.front() else {
                break;
            };
            if expiry.expires_at > now {
                break;
            }
            self.remove_repeat_suppression_expiry();
        }
    }

    pub(super) fn remove_repeat_suppression_expiry(&mut self) {
        let Some(expiry) = self.repeat_suppression_expirations.pop_front() else {
            return;
        };
        if self.last_user_enqueue.get(&expiry.user_id) == Some(&expiry.accepted_at) {
            self.last_user_enqueue.remove(&expiry.user_id);
        }
    }

    pub(super) fn claim_repeat_suppression_cleanup(&mut self) -> bool {
        if self.repeat_suppression_cleanup_scheduled
            || self.repeat_suppression_expirations.is_empty()
        {
            return false;
        }
        self.repeat_suppression_cleanup_scheduled = true;
        true
    }

    /// Executes one bounded cleanup turn. `now` comes from the background task
    /// in production and is injected by tests so idle expiry needs no new chat.
    pub(super) fn run_repeat_suppression_cleanup_turn(&mut self, now: Instant) -> Option<bool> {
        self.cleanup_expired_repeat_suppression_entries(now);
        let Some(expiry) = self.repeat_suppression_expirations.front() else {
            self.repeat_suppression_cleanup_scheduled = false;
            return None;
        };
        Some(expiry.expires_at <= now)
    }

    pub(super) fn cancel_in_flight(&mut self, reason: SkippedReason) -> bool {
        let Some(mut item) = self.in_flight.take() else {
            return false;
        };
        item.status = SpeechQueueItemStatus::Skipped;
        item.outcome = Some(SpeechQueueOutcome::skipped(reason, outcome::now_ms()));
        push_history(self, item);
        // The worker retains ownership until its physical send settles. Unique IDs
        // make that late completion a no-op, even if new items arrive meanwhile.
        true
    }

    pub(super) fn skip_current(&mut self) {
        if !self.cancel_in_flight(SkippedReason::UserSkip) {
            if let Some(mut item) = self.pending.pop_front() {
                item.status = SpeechQueueItemStatus::Skipped;
                item.outcome = Some(SpeechQueueOutcome::skipped(
                    SkippedReason::UserSkip,
                    outcome::now_ms(),
                ));
                push_history(self, item);
            }
        }
    }

    pub(super) fn make_pending_room(&mut self) -> bool {
        let mut dropped_any = false;
        while self.pending.len() + usize::from(self.in_flight.is_some()) >= DEFAULT_QUEUE_LIMIT {
            let Some(mut item) = self.pending.pop_front() else {
                break;
            };
            item.status = SpeechQueueItemStatus::Skipped;
            item.outcome = Some(SpeechQueueOutcome::skipped(
                SkippedReason::Overflow,
                outcome::now_ms(),
            ));
            push_history(self, item);
            dropped_any = true;
        }
        dropped_any
    }

    pub(super) fn clear_pending(&mut self) {
        self.cancel_in_flight(SkippedReason::Cleared);
        while let Some(mut item) = self.pending.pop_front() {
            item.status = SpeechQueueItemStatus::Skipped;
            item.outcome = Some(SpeechQueueOutcome::skipped(
                SkippedReason::Cleared,
                outcome::now_ms(),
            ));
            push_history(self, item);
        }
    }

    pub(super) fn remove_pending_item(&mut self, item_id: &str) -> bool {
        if self
            .in_flight
            .as_ref()
            .is_some_and(|item| item.id == item_id)
        {
            return self.cancel_in_flight(SkippedReason::Removed);
        }
        let Some(index) = self.pending.iter().position(|item| item.id == item_id) else {
            return false;
        };

        let mut item = self.pending.remove(index).expect("queue index checked");
        item.status = SpeechQueueItemStatus::Skipped;
        item.outcome = Some(SpeechQueueOutcome::skipped(
            SkippedReason::Removed,
            outcome::now_ms(),
        ));
        push_history(self, item);
        true
    }

    pub(super) fn dismiss_history_item(&mut self, item_id: &str) -> bool {
        let Some(index) = self.history.iter().position(|item| item.id == item_id) else {
            return false;
        };

        self.history.remove(index);
        true
    }

    pub(super) fn dismiss_history(&mut self) {
        self.history.clear();
    }

    pub(super) fn claim_worker(&mut self) -> bool {
        if self.is_processing
            || self.paused
            || self.controls_in_progress > 0
            || !self.has_auto_processable_item()
        {
            return false;
        }
        self.is_processing = true;
        true
    }

    pub(super) fn cancel_control_and_claim_worker(&mut self) -> bool {
        self.controls_in_progress = self.controls_in_progress.saturating_sub(1);
        self.claim_worker()
    }

    pub(super) fn has_auto_processable_item(&self) -> bool {
        self.in_flight.is_none()
            && self
                .pending
                .front()
                .is_some_and(|item| item.delivery_state == SpeechQueueDeliveryState::Ready)
    }

    pub(super) fn begin_next_request(&mut self) -> Option<SpeechRequest> {
        if self.controls_in_progress > 0 || self.in_flight.is_some() {
            return None;
        }
        let front = self.pending.front()?;
        if front.delivery_state != SpeechQueueDeliveryState::Ready {
            return None;
        }

        let mut item = self.pending.pop_front().expect("front checked");
        item.status = SpeechQueueItemStatus::Speaking;
        let request = SpeechRequest {
            id: item.id.clone(),
            source_message_id: item.source_message_id.clone(),
            text: item.text.clone(),
        };
        self.in_flight = Some(item);
        Some(request)
    }

    pub(super) fn reserve_next_request_after_dispatch_lock(&mut self) -> Option<SpeechRequest> {
        if self.paused {
            return None;
        }
        self.begin_next_request()
    }

    pub(super) fn complete_request(&mut self, request_id: &str) -> bool {
        if self
            .in_flight
            .as_ref()
            .is_none_or(|item| item.id != request_id)
        {
            return false;
        }
        let mut item = self.in_flight.take().expect("in-flight checked");
        item.status = SpeechQueueItemStatus::Spoken;
        item.outcome = None;
        push_history(self, item);
        true
    }

    #[cfg(test)]
    pub(super) fn fail_request(&mut self, request_id: &str) -> SpeechQueueFailureTransition {
        self.fail_request_with_retry(
            request_id,
            &SpeechFailure {
                code: FailureCode::ConnectTimeout,
                status: SpeechStatus::Disconnected,
                retryable: true,
                user_message: String::new(),
                detail: String::new(),
            },
        )
    }

    pub(super) fn fail_request_with_retry(
        &mut self,
        request_id: &str,
        failure: &SpeechFailure,
    ) -> SpeechQueueFailureTransition {
        if self
            .in_flight
            .as_ref()
            .is_none_or(|item| item.id != request_id)
        {
            return SpeechQueueFailureTransition::Ignored;
        }
        let mut item = self.in_flight.take().expect("in-flight checked");
        item.outcome = Some(SpeechQueueOutcome::error(failure, false, outcome::now_ms()));
        if failure.retryable && item.retry_count == 0 {
            item.retry_count = 1;
            item.status = SpeechQueueItemStatus::Queued;
            item.delivery_state = SpeechQueueDeliveryState::RetryScheduled;
            self.pending.push_front(item);
            return SpeechQueueFailureTransition::RetryScheduled;
        }
        item.status = SpeechQueueItemStatus::Error;
        item.delivery_state = SpeechQueueDeliveryState::RetryExhausted;
        push_history(self, item);
        SpeechQueueFailureTransition::RetryExhausted
    }

    pub(super) fn fail_after_acceptance(
        &mut self,
        request_id: &str,
        failure: &SpeechFailure,
    ) -> bool {
        if self
            .in_flight
            .as_ref()
            .is_none_or(|item| item.id != request_id)
        {
            return false;
        }
        let mut item = self.in_flight.take().expect("in-flight checked");
        item.status = SpeechQueueItemStatus::Error;
        item.outcome = Some(SpeechQueueOutcome::error(failure, true, outcome::now_ms()));
        item.delivery_state = SpeechQueueDeliveryState::RetryExhausted;
        // The adapter accepted the request, so consuming the automatic retry budget
        // prevents a duplicate utterance. A user may still explicitly retry it.
        item.retry_count = 1;
        push_history(self, item);
        true
    }

    pub(super) fn activate_scheduled_retry(&mut self, request_id: &str) -> bool {
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

    pub(super) fn retry_exhausted_item(&mut self, item_id: &str) -> bool {
        if !self.has_retry_capacity() {
            return false;
        }
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
        item.outcome = None;
        self.pending.push_back(item);
        true
    }

    pub(super) fn has_retry_capacity(&self) -> bool {
        self.pending.len() + usize::from(self.in_flight.is_some()) < DEFAULT_QUEUE_LIMIT
    }
}

#[cfg(any(feature = "app", test))]
#[derive(Default)]
pub(crate) struct QueueEnqueueOutcome {
    pub(super) warning: Option<String>,
    pub(super) should_spawn: bool,
    pub(super) should_schedule_cleanup: bool,
}

/// The same mutation path is used by Tauri and the deterministic scheduler
/// harness. The caller takes the settings snapshot/formatter before queue lock.
#[cfg(any(feature = "app", test))]
pub(crate) fn enqueue_message(
    queue: &mut SpeechQueueState,
    speech_settings: &SpeechSettings,
    formatter: &SpeechFormatter,
    message: ChatMessage,
    now: Instant,
) -> QueueEnqueueOutcome {
    let mut outcome = QueueEnqueueOutcome::default();
    if !speech_settings.auto_speak {
        let id = next_queue_id(queue);
        push_history(
            queue,
            SpeechQueueItem {
                id,
                source_message_id: Some(message.id),
                user_display_name: message.user_display_name,
                text: message.text,
                status: SpeechQueueItemStatus::Skipped,
                retry_count: 0,
                delivery_state: SpeechQueueDeliveryState::Ready,
                outcome: Some(SpeechQueueOutcome::skipped(
                    SkippedReason::AutoSpeakDisabled,
                    outcome::now_ms(),
                )),
            },
        );
        return outcome;
    }
    if let Some(warning_message) = suppress_repeated_message(queue, speech_settings, &message, now)
    {
        outcome.warning = Some(warning_message);
        return outcome;
    }

    let formatted_text = match formatter.format_chat_message(&message) {
        SpeechFormatDecision::Speak(text) => text,
        SpeechFormatDecision::Blocked(reason) => {
            let id = next_queue_id(queue);
            let warning_message = format!("[{id}] {}", reason.message());
            push_history(
                queue,
                SpeechQueueItem {
                    id,
                    source_message_id: Some(message.id.clone()),
                    user_display_name: message.user_display_name.clone(),
                    text: message.text.clone(),
                    status: SpeechQueueItemStatus::Blocked,
                    retry_count: 0,
                    delivery_state: SpeechQueueDeliveryState::Ready,
                    outcome: Some(SpeechQueueOutcome::blocked(reason, outcome::now_ms())),
                },
            );
            outcome.warning = Some(warning_message);
            return outcome;
        }
    };

    if speech_settings.repeat_suppression_seconds > 0 {
        queue.record_user_enqueue(message.user_id.clone(), now);
        outcome.should_schedule_cleanup = queue.claim_repeat_suppression_cleanup();
    }
    if queue.make_pending_room() {
        outcome.warning = queue.history.front().map(|item| {
            format!(
                "[{}] 読み上げキューが上限に達したため、古い未読チャットを落としました。",
                item.id
            )
        });
    }

    let item = SpeechQueueItem {
        id: next_queue_id(queue),
        source_message_id: Some(message.id),
        user_display_name: message.user_display_name,
        text: formatted_text,
        status: SpeechQueueItemStatus::Queued,
        retry_count: 0,
        delivery_state: SpeechQueueDeliveryState::Ready,
        outcome: None,
    };
    queue.pending.push_back(item);
    outcome.should_spawn = queue.claim_worker();

    outcome
}

pub(super) fn is_repeat_suppressed(
    last_enqueue: Option<Instant>,
    now: Instant,
    repeat_suppression_seconds: u64,
) -> bool {
    repeat_suppression_seconds > 0
        && last_enqueue.is_some_and(|last_enqueue| {
            now.duration_since(last_enqueue) < Duration::from_secs(repeat_suppression_seconds)
        })
}

pub(super) fn suppress_repeated_message(
    queue: &mut SpeechQueueState,
    settings: &SpeechSettings,
    message: &ChatMessage,
    now: Instant,
) -> Option<String> {
    queue.prepare_repeat_suppression(message, settings.repeat_suppression_seconds, now);
    if !is_repeat_suppressed(
        queue.last_user_enqueue.get(&message.user_id).copied(),
        now,
        u64::from(settings.repeat_suppression_seconds),
    ) {
        return None;
    }

    let id = next_queue_id(queue);
    let warning_message = format!("[{id}] {}", BlockedReason::RepeatSuppressed.message());
    push_history(
        queue,
        SpeechQueueItem {
            id,
            source_message_id: Some(message.id.clone()),
            user_display_name: message.user_display_name.clone(),
            text: message.text.clone(),
            status: SpeechQueueItemStatus::Blocked,
            retry_count: 0,
            delivery_state: SpeechQueueDeliveryState::Ready,
            outcome: Some(SpeechQueueOutcome::blocked(
                BlockedReason::RepeatSuppressed,
                outcome::now_ms(),
            )),
        },
    );
    Some(warning_message)
}

pub(super) fn next_queue_id(queue: &mut SpeechQueueState) -> String {
    let id = queue.next_id;
    queue.next_id = queue.next_id.saturating_add(1);
    format!("speech-{id}")
}

pub(super) fn push_history(queue: &mut SpeechQueueState, item: SpeechQueueItem) {
    queue.history.push_front(item);
    while queue.history.len() > DEFAULT_HISTORY_LIMIT {
        queue.history.pop_back();
    }
}
