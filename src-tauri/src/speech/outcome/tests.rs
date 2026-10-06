use super::*;
use crate::app_events::{SpeechQueueItemStatus, SpeechStatus};
use crate::settings::AppSettings;
use crate::speech::{
    enqueue_message, SpeechFormatter, SpeechFormatterOptions, SpeechQueueFailureTransition,
    SpeechQueueState, DEFAULT_HISTORY_LIMIT, DEFAULT_QUEUE_LIMIT,
};
use crate::twitch::{ChatMessage, Platform};
use std::time::Instant;

fn message(id: &str, text: &str) -> ChatMessage {
    ChatMessage {
        id: id.into(),
        platform: Platform::Twitch,
        channel_id: "channel".into(),
        channel_login: "channel".into(),
        user_id: "viewer".into(),
        user_login: "viewer".into(),
        user_display_name: "viewer".into(),
        text: text.into(),
        fragments: vec![],
        badges: vec![],
        received_at: chrono::Utc::now(),
        connection_generation: Some(1),
    }
}

fn enqueue(queue: &mut SpeechQueueState, text: &str) -> String {
    let mut settings = AppSettings::default().speech;
    settings.repeat_suppression_seconds = 0;
    let id = format!("chat-{}", queue.next_id);
    enqueue_message(
        queue,
        &settings,
        &SpeechFormatter::new(SpeechFormatterOptions::from(&settings)),
        message(&id, text),
        Instant::now(),
    );
    queue.pending.back().unwrap().id.clone()
}

fn failure(code: FailureCode, retryable: bool) -> SpeechFailure {
    SpeechFailure {
        code,
        status: SpeechStatus::Error,
        retryable,
        user_message: "SECRET_TOKEN_AND_HOST".into(),
        detail: "SECRET_TOKEN_AND_HOST".into(),
    }
}

#[test]
fn all_reason_codes_share_the_exact_typescript_fixture_and_never_copy_secrets() {
    let at = 1_790_000_000_000;
    let mut outcomes = [
        BlockedReason::RepeatSuppressed,
        BlockedReason::BlockedUser,
        BlockedReason::BlockedWord,
        BlockedReason::BlockedUrl,
        BlockedReason::EmptyAfterFormatting,
    ]
    .into_iter()
    .map(|code| SpeechQueueOutcome::blocked(code, at))
    .collect::<Vec<_>>();
    outcomes.extend(
        [
            SkippedReason::Overflow,
            SkippedReason::UserSkip,
            SkippedReason::Removed,
            SkippedReason::Cleared,
            SkippedReason::AutoSpeakDisabled,
        ]
        .into_iter()
        .map(|code| SpeechQueueOutcome::skipped(code, at)),
    );
    for (code, retryable, accepted) in [
        (FailureCode::Configuration, false, false),
        (FailureCode::ConnectionRefused, true, false),
        (FailureCode::ConnectTimeout, true, false),
        (FailureCode::ConnectFailed, true, false),
        (FailureCode::ConnectionLost, false, false),
        (FailureCode::PermissionDenied, false, false),
        (FailureCode::WriteTimeout, false, false),
        (FailureCode::WriteFailed, false, false),
        (FailureCode::ResponseTimeout, false, true),
        (FailureCode::ResponseFailed, false, true),
        (FailureCode::ProtocolMismatch, false, true),
        (FailureCode::Unknown, false, false),
    ] {
        outcomes.push(SpeechQueueOutcome::error(
            &failure(code, retryable),
            accepted,
            at,
        ));
    }
    let serialized = serde_json::to_value(&outcomes).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../src/tauri/fixtures/queue-outcomes.json"
    ))
    .unwrap();
    assert_eq!(serialized, fixture);
    assert!(!serialized.to_string().contains("SECRET_TOKEN_AND_HOST"));
}

#[test]
fn actual_formatter_and_repeat_paths_keep_safe_reasons_after_unrelated_updates_and_reload() {
    for (text, blocked_users, blocked_words, url_handling, expected) in [
        (
            "hello",
            vec!["viewer"],
            vec![],
            crate::settings::UrlHandling::Replace,
            BlockedReason::BlockedUser,
        ),
        (
            "SECRET_NG_WORD",
            vec![],
            vec!["SECRET_NG_WORD"],
            crate::settings::UrlHandling::Replace,
            BlockedReason::BlockedWord,
        ),
        (
            "https://example.com",
            vec![],
            vec![],
            crate::settings::UrlHandling::Block,
            BlockedReason::BlockedUrl,
        ),
        (
            "\0\t",
            vec![],
            vec![],
            crate::settings::UrlHandling::Replace,
            BlockedReason::EmptyAfterFormatting,
        ),
    ] {
        let mut queue = SpeechQueueState::default();
        let mut settings = AppSettings::default().speech;
        settings.blocked_users = blocked_users.into_iter().map(String::from).collect();
        settings.blocked_words = blocked_words.into_iter().map(String::from).collect();
        settings.url_handling = url_handling;
        let result = enqueue_message(
            &mut queue,
            &settings,
            &SpeechFormatter::new(SpeechFormatterOptions::from(&settings)),
            message("blocked-source", text),
            Instant::now(),
        );
        let item = queue.history.front().unwrap().clone();
        assert!(result.warning.unwrap().contains(&format!("[{}]", item.id)));
        assert!(
            matches!(item.outcome, Some(SpeechQueueOutcome::Blocked { reason_code, .. }) if reason_code == expected)
        );
        assert!(!serde_json::to_string(&item.outcome)
            .unwrap()
            .contains("SECRET_NG_WORD"));
        enqueue(&mut queue, "unrelated");
        for _ in 0..3 {
            let snapshot = queue.snapshot(None);
            let restored = snapshot
                .items
                .iter()
                .find(|event| event.id == item.id)
                .unwrap();
            assert_eq!(restored.outcome, item.outcome);
            assert_eq!(
                restored.source_message_id.as_deref(),
                Some("blocked-source")
            );
            assert!(snapshot.warning.is_none());
        }
    }
    let mut queue = SpeechQueueState::default();
    let settings = AppSettings::default().speech;
    let formatter = SpeechFormatter::new(SpeechFormatterOptions::from(&settings));
    let now = Instant::now();
    enqueue_message(
        &mut queue,
        &settings,
        &formatter,
        message("first", "first"),
        now,
    );
    enqueue_message(
        &mut queue,
        &settings,
        &formatter,
        message("repeat", "second"),
        now,
    );
    assert!(matches!(
        queue.history.back().unwrap().outcome,
        Some(SpeechQueueOutcome::Blocked {
            reason_code: BlockedReason::RepeatSuppressed,
            ..
        })
    ));
}

#[test]
fn skip_remove_clear_and_overflow_keep_distinct_causes_with_bounded_history() {
    let mut queue = SpeechQueueState::default();
    enqueue(&mut queue, "skip");
    queue.begin_next_request().unwrap();
    queue.skip_current();
    let removed = enqueue(&mut queue, "remove");
    assert!(queue.remove_pending_item(&removed));
    enqueue(&mut queue, "clear in flight");
    queue.begin_next_request().unwrap();
    enqueue(&mut queue, "clear pending");
    queue.clear_pending();
    let causes = queue
        .history
        .iter()
        .map(|item| match item.outcome.as_ref().unwrap() {
            SpeechQueueOutcome::Skipped { reason_code, .. } => *reason_code,
            _ => panic!("skipped outcome"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        causes,
        [
            SkippedReason::Cleared,
            SkippedReason::Cleared,
            SkippedReason::Removed,
            SkippedReason::UserSkip
        ]
    );
    for _ in 0..(DEFAULT_QUEUE_LIMIT + DEFAULT_HISTORY_LIMIT * 2) {
        enqueue(&mut queue, "overflow");
    }
    assert_eq!(queue.pending.len(), DEFAULT_QUEUE_LIMIT);
    assert_eq!(queue.history.len(), DEFAULT_HISTORY_LIMIT);
    let snapshot = queue.snapshot(None);
    assert_eq!(
        snapshot.items.len(),
        DEFAULT_QUEUE_LIMIT + DEFAULT_HISTORY_LIMIT
    );
    assert!(queue.history.iter().all(|item| matches!(
        item.outcome,
        Some(SpeechQueueOutcome::Skipped {
            reason_code: SkippedReason::Overflow,
            ..
        })
    )));
}

#[test]
fn typed_failure_survives_retry_and_reload_then_manual_retry_and_success_clear_it() {
    let mut queue = SpeechQueueState::default();
    let id = enqueue(&mut queue, "failure");
    queue.begin_next_request().unwrap();
    let first = failure(FailureCode::ConnectTimeout, true);
    assert_eq!(
        queue.fail_request_with_retry(&id, &first),
        SpeechQueueFailureTransition::RetryScheduled
    );
    assert!(queue.pending.front().unwrap().outcome.is_some());
    assert!(queue.activate_scheduled_retry(&id));
    queue.begin_next_request().unwrap();
    let second = failure(FailureCode::ConnectionRefused, true);
    assert_eq!(
        queue.fail_request_with_retry(&id, &second),
        SpeechQueueFailureTransition::RetryExhausted
    );
    let saved = queue.history.front().unwrap().outcome.clone();
    assert!(matches!(
        saved,
        Some(SpeechQueueOutcome::Error {
            reason_code: FailureCode::ConnectionRefused,
            ..
        })
    ));
    assert_eq!(queue.snapshot(None).items[0].outcome, saved);
    assert!(queue.retry_exhausted_item(&id));
    assert!(queue.pending.front().unwrap().outcome.is_none());
    queue.begin_next_request().unwrap();
    assert!(queue.complete_request(&id));
    assert!(queue.history.front().unwrap().outcome.is_none());
    assert_eq!(
        queue.history.front().unwrap().status,
        SpeechQueueItemStatus::Spoken
    );
}

#[test]
fn overflow_warning_correlates_with_the_newly_skipped_item_not_an_older_history_entry() {
    let mut queue = SpeechQueueState::default();
    enqueue(&mut queue, "old skipped");
    queue.skip_current();
    for _ in 0..DEFAULT_QUEUE_LIMIT {
        enqueue(&mut queue, "pending");
    }
    let dropped_id = queue.pending.front().unwrap().id.clone();
    let mut settings = AppSettings::default().speech;
    settings.repeat_suppression_seconds = 0;
    let result = enqueue_message(
        &mut queue,
        &settings,
        &SpeechFormatter::new(SpeechFormatterOptions::from(&settings)),
        message("overflow-new", "new"),
        Instant::now(),
    );
    assert!(result.warning.unwrap().contains(&format!("[{dropped_id}]")));
    assert_eq!(queue.history.front().unwrap().id, dropped_id);
    assert!(matches!(
        queue.history.front().unwrap().outcome,
        Some(SpeechQueueOutcome::Skipped {
            reason_code: SkippedReason::Overflow,
            ..
        })
    ));
}

#[test]
fn accepted_uncertain_delivery_and_cancelled_late_failure_preserve_correct_outcome() {
    let mut queue = SpeechQueueState::default();
    let id = enqueue(&mut queue, "accepted");
    queue.begin_next_request().unwrap();
    assert!(queue.fail_after_acceptance(&id, &failure(FailureCode::ResponseTimeout, true)));
    match queue.history.front().unwrap().outcome.as_ref().unwrap() {
        SpeechQueueOutcome::Error { details, .. } => {
            assert!(!details.retryable);
            assert_eq!(details.recovery_action, RecoveryAction::ConfirmDelivery);
            assert!(details.occurred_at_ms > 0);
        }
        _ => panic!("error outcome"),
    }
    let cancelled = enqueue(&mut queue, "cancelled");
    queue.begin_next_request().unwrap();
    queue.clear_pending();
    let before = queue.history.front().unwrap().outcome.clone();
    assert!(matches!(
        before,
        Some(SpeechQueueOutcome::Skipped {
            reason_code: SkippedReason::Cleared,
            ..
        })
    ));
    assert!(!queue.fail_after_acceptance(&cancelled, &failure(FailureCode::ResponseFailed, false)));
    assert_eq!(
        queue.fail_request_with_retry(&cancelled, &failure(FailureCode::Unknown, false)),
        SpeechQueueFailureTransition::Ignored
    );
    assert_eq!(queue.history.front().unwrap().outcome, before);
}

#[test]
fn automatic_speech_uses_the_captured_setting_and_records_exclusions_in_snapshots() {
    let mut queue = SpeechQueueState::default();
    let mut live_settings = AppSettings::default().speech;
    live_settings.repeat_suppression_seconds = 0;
    live_settings.auto_speak = false;
    let captured_off = live_settings.clone();
    live_settings.auto_speak = true;
    let skipped = enqueue_message(
        &mut queue,
        &captured_off,
        &SpeechFormatter::new(SpeechFormatterOptions::from(&captured_off)),
        message("received-off", "first"),
        Instant::now(),
    );
    assert!(!skipped.should_spawn && !skipped.should_schedule_cleanup);
    assert!(skipped.warning.is_none());
    assert!(queue.pending.is_empty() && queue.in_flight.is_none());
    assert!(queue.last_user_enqueue.is_empty());
    let snapshot = queue.snapshot(None);
    assert_eq!(snapshot.queued_count, 0);
    let excluded = &snapshot.items[0];
    assert_eq!(excluded.source_message_id.as_deref(), Some("received-off"));
    assert_eq!(excluded.status, SpeechQueueItemStatus::Skipped);
    assert!(matches!(
        excluded.outcome,
        Some(SpeechQueueOutcome::Skipped {
            reason_code: SkippedReason::AutoSpeakDisabled,
            ..
        })
    ));

    // OFF after capture cannot retroactively cancel an accepted ON decision.
    let captured_on = live_settings.clone();
    live_settings.auto_speak = false;
    let accepted = enqueue_message(
        &mut queue,
        &captured_on,
        &SpeechFormatter::new(SpeechFormatterOptions::from(&captured_on)),
        message("received-on", "second"),
        Instant::now(),
    );
    assert!(accepted.should_spawn);
    assert_eq!(queue.pending.len(), 1);
    assert_eq!(
        queue.pending[0].source_message_id.as_deref(),
        Some("received-on")
    );
    assert_eq!(queue.history.len(), 1);

    // Exclusions keep bounded terminal history and never become pending later.
    for index in 0..=DEFAULT_HISTORY_LIMIT {
        enqueue_message(
            &mut queue,
            &live_settings,
            &SpeechFormatter::new(SpeechFormatterOptions::from(&live_settings)),
            message(&format!("off-{index}"), "third"),
            Instant::now(),
        );
    }
    assert_eq!(queue.history.len(), DEFAULT_HISTORY_LIMIT);
    assert_eq!(queue.pending.len(), 1);
    let restored = serde_json::to_value(crate::app_events::speech_queue_updated_event(
        queue.snapshot(None),
    ))
    .unwrap();
    assert_eq!(restored["queuedCount"], 1);
    assert_eq!(
        restored["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["outcome"]["reasonCode"] == "autoSpeakDisabled")
            .count(),
        DEFAULT_HISTORY_LIMIT
    );
}
