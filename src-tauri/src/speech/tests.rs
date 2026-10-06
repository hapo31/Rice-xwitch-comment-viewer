use super::*;
use crate::app_events::SpeechQueuePhase;
use crate::twitch::Platform;

#[test]
fn speech_request_rejects_each_unsupported_override_instead_of_ignoring_it() {
    let base = serde_json::json!({"id": "1", "sourceMessageId": null, "text": "こんにちは"});
    assert!(serde_json::from_value::<SpeechRequest>(base.clone()).is_ok());
    for field in ["voice", "speed", "tone", "volume"] {
        for value in [
            serde_json::json!(100),
            serde_json::json!("default"),
            serde_json::Value::Null,
        ] {
            let mut request = base.clone();
            request[field] = value;
            let error = serde_json::from_value::<SpeechRequest>(request).unwrap_err();
            assert!(error.to_string().contains("unknown field"));
            assert!(error.to_string().contains(field));
        }
    }
    let serialized =
        serde_json::to_value(serde_json::from_value::<SpeechRequest>(base.clone()).unwrap())
            .unwrap();
    assert_eq!(serialized, base);
}

fn queued_item(id: &str) -> SpeechQueueItem {
    SpeechQueueItem {
        id: id.to_string(),
        source_message_id: None,
        user_display_name: "viewer".to_string(),
        text: "こんにちは".to_string(),
        status: SpeechQueueItemStatus::Queued,
        retry_count: 0,
        delivery_state: SpeechQueueDeliveryState::Ready,
        outcome: None,
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
        outcome: None,
    }
}

#[test]
fn speech_settings_control_repeat_suppression_in_the_queue_path() {
    let now = Instant::now();
    let message = chat("連投テスト");
    let mut settings = crate::settings::AppSettings::default().speech;
    let mut queue = SpeechQueueState::default();

    settings.repeat_suppression_seconds = 0;
    queue.record_user_enqueue(message.user_id.clone(), now);
    assert!(suppress_repeated_message(&mut queue, &settings, &message, now).is_none());
    assert!(queue.history.is_empty());
    assert!(queue.last_user_enqueue.is_empty());

    settings.repeat_suppression_seconds = 1;
    let accepted_at = now - Duration::from_millis(999);
    queue.record_user_enqueue(message.user_id.clone(), accepted_at);
    assert!(suppress_repeated_message(&mut queue, &settings, &message, now).is_some());
    assert_eq!(queue.history.len(), 1);
    assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Blocked);
    assert_eq!(queue.last_user_enqueue[&message.user_id], accepted_at);

    queue.clear_repeat_suppression_entries();
    queue.history.clear();
    queue.record_user_enqueue(message.user_id.clone(), now - Duration::from_secs(1));
    assert!(suppress_repeated_message(&mut queue, &settings, &message, now).is_none());

    settings.repeat_suppression_seconds = 2;
    queue.clear_repeat_suppression_entries();
    queue.record_user_enqueue(message.user_id.clone(), now - Duration::from_millis(1_999));
    assert!(suppress_repeated_message(&mut queue, &settings, &message, now).is_some());
    queue.clear_repeat_suppression_entries();
    queue.history.clear();
    queue.record_user_enqueue(message.user_id.clone(), now - Duration::from_secs(2));
    assert!(suppress_repeated_message(&mut queue, &settings, &message, now).is_none());
}

#[test]
fn repeat_suppression_expires_entries_at_the_maximum_setting_boundary() {
    let now = Instant::now();
    let message = chat("最大期間の境界");
    let mut settings = crate::settings::AppSettings::default().speech;
    settings.repeat_suppression_seconds = 30;
    let mut queue = SpeechQueueState::default();

    assert!(suppress_repeated_message(&mut queue, &settings, &message, now).is_none());
    queue.record_user_enqueue(
        message.user_id.clone(),
        now - MAX_REPEAT_SUPPRESSION_WINDOW + Duration::from_millis(1),
    );
    assert!(suppress_repeated_message(&mut queue, &settings, &message, now).is_some());

    queue.clear_repeat_suppression_entries();
    queue.record_user_enqueue(message.user_id.clone(), now - MAX_REPEAT_SUPPRESSION_WINDOW);
    assert!(suppress_repeated_message(&mut queue, &settings, &message, now).is_none());
    assert!(queue.last_user_enqueue.is_empty());
    assert!(queue.repeat_suppression_expirations.is_empty());
}

#[test]
fn repeat_suppression_idle_cleanup_releases_expired_entries_with_an_injected_clock() {
    let accepted_at = Instant::now();
    let message = chat("待機中に期限切れ");
    let settings = crate::settings::AppSettings::default().speech;
    let mut queue = SpeechQueueState::default();

    assert!(suppress_repeated_message(&mut queue, &settings, &message, accepted_at).is_none());
    queue.record_user_enqueue(message.user_id.clone(), accepted_at);
    assert!(queue.claim_repeat_suppression_cleanup());

    // No incoming message is required for the scheduled cleanup path: its
    // clock value is injected here just as it is by the background task.
    assert_eq!(
        queue.run_repeat_suppression_cleanup_turn(accepted_at),
        Some(false)
    );
    assert_eq!(
        queue.run_repeat_suppression_cleanup_turn(accepted_at + MAX_REPEAT_SUPPRESSION_WINDOW),
        None
    );
    assert!(queue.last_user_enqueue.is_empty());
    assert!(queue.repeat_suppression_expirations.is_empty());
}

#[test]
fn old_repeat_suppression_expiry_cannot_remove_a_newer_user_timestamp() {
    let now = Instant::now();
    let message = chat("設定変更後の受理");
    let settings = crate::settings::AppSettings::default().speech;
    let mut queue = SpeechQueueState::default();

    assert!(suppress_repeated_message(&mut queue, &settings, &message, now).is_none());
    queue.record_user_enqueue(message.user_id.clone(), now - MAX_REPEAT_SUPPRESSION_WINDOW);
    let newer_accepted_at = now - Duration::from_secs(1);
    queue.record_user_enqueue(message.user_id.clone(), newer_accepted_at);

    assert!(suppress_repeated_message(&mut queue, &settings, &message, now).is_some());
    assert_eq!(
        queue.last_user_enqueue.get(&message.user_id),
        Some(&newer_accepted_at)
    );
    assert_eq!(queue.repeat_suppression_expirations.len(), 1);
}

#[test]
fn repeat_suppression_resets_when_the_chat_session_changes() {
    let now = Instant::now();
    let mut first_message = chat("最初のコメント");
    first_message.connection_generation = Some(1);
    let mut next_session_message = first_message.clone();
    next_session_message.id = "next-session".to_string();
    next_session_message.connection_generation = Some(2);
    let settings = crate::settings::AppSettings::default().speech;
    let mut queue = SpeechQueueState::default();

    assert!(suppress_repeated_message(&mut queue, &settings, &first_message, now).is_none());
    queue.record_user_enqueue(first_message.user_id.clone(), now);
    assert!(suppress_repeated_message(&mut queue, &settings, &first_message, now).is_some());
    assert!(suppress_repeated_message(&mut queue, &settings, &next_session_message, now).is_none());
    assert!(queue.last_user_enqueue.is_empty());
    assert_eq!(
        queue.repeat_suppression_scope,
        Some(RepeatSuppressionScope {
            channel_id: next_session_message.channel_id.clone(),
            connection_generation: Some(2),
        })
    );

    queue.record_user_enqueue(next_session_message.user_id.clone(), now);
    let mut other_channel_message = next_session_message.clone();
    other_channel_message.id = "other-channel".to_string();
    other_channel_message.channel_id = "other-channel-id".to_string();
    assert!(
        suppress_repeated_message(&mut queue, &settings, &other_channel_message, now).is_none()
    );
    assert!(queue.last_user_enqueue.is_empty());
    assert_eq!(
        queue.repeat_suppression_scope,
        Some(RepeatSuppressionScope {
            channel_id: "other-channel-id".to_string(),
            connection_generation: Some(2),
        })
    );
}

#[test]
fn repeat_suppression_keeps_large_unique_input_bounded_without_full_map_cleanup() {
    let now = Instant::now();
    let message = chat("期限切れを掃除します");
    let settings = crate::settings::AppSettings::default().speech;
    let mut queue = SpeechQueueState::default();

    assert!(suppress_repeated_message(&mut queue, &settings, &message, now).is_none());
    for index in 0..MAX_REPEAT_SUPPRESSION_ENTRIES + 100 {
        queue.record_user_enqueue(format!("viewer-{index}"), now);
    }
    assert_eq!(
        queue.last_user_enqueue.len(),
        MAX_REPEAT_SUPPRESSION_ENTRIES
    );
    assert_eq!(
        queue.repeat_suppression_expirations.len(),
        MAX_REPEAT_SUPPRESSION_ENTRIES
    );

    for _ in 0..MAX_REPEAT_SUPPRESSION_ENTRIES / REPEAT_SUPPRESSION_CLEANUP_BATCH {
        assert!(suppress_repeated_message(
            &mut queue,
            &settings,
            &message,
            now + MAX_REPEAT_SUPPRESSION_WINDOW
        )
        .is_none());
    }
    assert!(queue.last_user_enqueue.is_empty());
    assert!(queue.repeat_suppression_expirations.is_empty());
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
fn uncertain_delivery_failure_is_not_scheduled_for_automatic_retry() {
    let mut queue = SpeechQueueState::default();
    queue.pending.push_back(queued_item("uncertain"));
    let request = queue.reserve_next_request_after_dispatch_lock().unwrap();
    let failure = SpeechFailure {
        code: FailureCode::WriteTimeout,
        status: crate::app_events::SpeechStatus::Disconnected,
        retryable: false,
        user_message: "送信の到達が不明です。".to_string(),
        detail: "fake write timeout".to_string(),
    };
    assert_eq!(
        queue.fail_request_with_retry(&request.id, &failure),
        SpeechQueueFailureTransition::RetryExhausted
    );
    assert!(queue.pending.is_empty());
    assert!(queue.in_flight.is_none());
    assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Error);
    assert_eq!(queue.history[0].retry_count, 0);
}

#[cfg(feature = "app")]
#[test]
fn failed_history_waits_for_manual_retry_without_stopping_later_pending_work() {
    let mut queue = SpeechQueueState::default();
    queue
        .history
        .push_back(history_item("failed", SpeechQueueItemStatus::Error));
    assert_eq!(
        queue_event_snapshot(&queue, None).phase,
        SpeechQueuePhase::Error
    );
    queue.pending.push_back(queued_item("later"));
    assert_eq!(
        queue_event_snapshot(&queue, None).phase,
        SpeechQueuePhase::Idle
    );
    assert_eq!(
        queue.reserve_next_request_after_dispatch_lock().unwrap().id,
        "later"
    );
    assert_eq!(
        queue_event_snapshot(&queue, None).phase,
        SpeechQueuePhase::Speaking
    );
    queue.paused = true;
    assert_eq!(
        queue_event_snapshot(&queue, None).phase,
        SpeechQueuePhase::Paused
    );
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
        received_at: DateTime::parse_from_rfc3339("2026-05-23T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        connection_generation: None,
    }
}

fn chat_from(display_name: &str, text: &str) -> ChatMessage {
    ChatMessage {
        user_display_name: display_name.to_string(),
        ..chat(text)
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
fn formatter_applies_url_rules_to_embedded_urls() {
    let cases = [
        ("standalone", "https://example.com/path", "URL省略"),
        (
            "uppercase HTTP and punycode",
            "HTTP://XN--R8JZ45G.XN--ZCKZAH/path",
            "URL省略",
        ),
        (
            "Japanese text before URL",
            "詳しくはhttps://example.com/pathです",
            "詳しくはURL省略です",
        ),
        (
            "parentheses and Japanese quotes",
            "(https://example.com)と「www.example.com/path」",
            "(URL省略)と「URL省略」",
        ),
        (
            "ASCII quotes",
            "\"https://example.com\" と'www.example.com/path'",
            "\"URL省略\" と'URL省略'",
        ),
        (
            "balanced path parentheses",
            "(https://example.com/a_(b))です",
            "(URL省略)です",
        ),
        (
            "balanced path and query parentheses",
            "https://example.com/a_(b)?q=(c)",
            "URL省略",
        ),
        (
            "opening parenthesis after URL",
            "https://example.com(続き)",
            "URL省略(続き)",
        ),
        (
            "opening bracket after URL",
            "https://example.com[続き]",
            "URL省略[続き]",
        ),
        (
            "ASCII parenthesized prose after URL",
            "https://example.com(note)",
            "URL省略(note)",
        ),
        (
            "ASCII bracketed prose after URL",
            "https://example.com[notes]",
            "URL省略[notes]",
        ),
        (
            "opening quote after URL",
            "https://example.com'続き'",
            "URL省略'続き'",
        ),
        (
            "valid bracketed IPv6 authority",
            "https://[2001:db8::1]/path",
            "URL省略",
        ),
        (
            "sentence punctuation",
            "https://example.com/path,続きです。",
            "URL省略,続きです。",
        ),
        (
            "multiple URLs",
            "https://one.example/a とwww.two.example/b!",
            "URL省略 とURL省略!",
        ),
    ];

    let replace_formatter = SpeechFormatter::new(SpeechFormatterOptions {
        read_user_name: false,
        escape_bouyomi_tags: false,
        ..SpeechFormatterOptions::default()
    });
    let block_formatter = SpeechFormatter::new(SpeechFormatterOptions {
        read_user_name: false,
        replace_urls: false,
        block_urls: true,
        escape_bouyomi_tags: false,
        ..SpeechFormatterOptions::default()
    });

    for (case_name, input, expected) in cases {
        assert_eq!(
            replace_formatter.format_chat_message(&chat(input)),
            SpeechFormatDecision::Speak(expected.to_string()),
            "replace: {case_name}"
        );
        assert_eq!(
            block_formatter.format_chat_message(&chat(input)),
            SpeechFormatDecision::Blocked(BlockedReason::BlockedUrl),
            "block: {case_name}"
        );
    }
}

#[test]
fn formatter_does_not_detect_incomplete_or_unrelated_url_like_text() {
    let replace_formatter = SpeechFormatter::new(SpeechFormatterOptions {
        read_user_name: false,
        escape_bouyomi_tags: false,
        ..SpeechFormatterOptions::default()
    });
    let block_formatter = SpeechFormatter::new(SpeechFormatterOptions {
        read_user_name: false,
        replace_urls: false,
        block_urls: true,
        escape_bouyomi_tags: false,
        ..SpeechFormatterOptions::default()
    });

    for (case_name, input) in [
        ("incomplete http prefix", "https:// を確認"),
        ("incomplete www prefix", "www. を確認"),
        ("incomplete bracketed IPv6", "https://[ を確認"),
        ("empty bracketed IPv6", "https://[] を確認"),
        ("invalid bracketed IPv6", "https://[oops] を確認"),
        ("invalid port", "https://example.com:65536 を確認"),
        (
            "www domain inside email",
            "連絡先 user@www.example.com です",
        ),
        ("scheme inside ASCII identifier", "abchttps://example.com"),
        ("ordinary text", "httpとwwwだけです"),
    ] {
        assert_eq!(
            replace_formatter.format_chat_message(&chat(input)),
            SpeechFormatDecision::Speak(input.to_string()),
            "replace: {case_name}"
        );
        assert_eq!(
            block_formatter.format_chat_message(&chat(input)),
            SpeechFormatDecision::Speak(input.to_string()),
            "block: {case_name}"
        );
    }
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
                    SpeechFormatDecision::Blocked(BlockedReason::EmptyAfterFormatting),
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
        SpeechFormatDecision::Speak("1234…".to_string())
    );
}

#[test]
fn formatter_applies_max_length_to_final_utterance() {
    let cases = [
        ("minimum", 1, "viewer", "本文", "…"),
        (
            "multibyte boundary",
            5,
            "viewer",
            "あいうえおか",
            "あいうえ…",
        ),
        ("user name prefix", 10, "viewer", "hello", "viewer。he…"),
        ("long display name", 5, "長い表示名", "本文", "長い表示…"),
    ];

    for (case_name, max_comment_length, display_name, body, expected) in cases {
        let formatter = SpeechFormatter::new(SpeechFormatterOptions {
            read_user_name: case_name != "multibyte boundary",
            max_comment_length,
            ..SpeechFormatterOptions::default()
        });

        let SpeechFormatDecision::Speak(text) =
            formatter.format_chat_message(&chat_from(display_name, body))
        else {
            panic!("{case_name}: expected speak decision");
        };
        assert_eq!(text, expected, "{case_name}");
        assert!(
            text.chars().count() <= max_comment_length,
            "{case_name}: final utterance exceeds its character budget"
        );
    }
}

#[test]
fn formatter_honors_default_and_maximum_length_boundaries() {
    for max_comment_length in [DEFAULT_MAX_COMMENT_LENGTH, 500] {
        let formatter = SpeechFormatter::new(SpeechFormatterOptions {
            read_user_name: true,
            max_comment_length,
            ..SpeechFormatterOptions::default()
        });
        let body = "あ".repeat(max_comment_length + 10);

        let SpeechFormatDecision::Speak(text) =
            formatter.format_chat_message(&chat_from("配信視聴者", &body))
        else {
            panic!("expected speak decision for max length {max_comment_length}");
        };
        assert_eq!(text.chars().count(), max_comment_length);
        assert!(text.ends_with('…'));
    }
}

#[test]
fn formatter_applies_url_and_ng_rules_before_final_truncation() {
    let replace_formatter = SpeechFormatter::new(SpeechFormatterOptions {
        read_user_name: false,
        max_comment_length: 4,
        ..SpeechFormatterOptions::default()
    });
    assert_eq!(
        replace_formatter.format_chat_message(&chat("https://example.com/path")),
        SpeechFormatDecision::Speak("URL…".to_string())
    );

    // Keep the embedded-URL behavior from Issue #61 while applying the
    // final utterance limit afterwards. Truncating the source text first
    // would leave a partial URL instead of the URL replacement.
    let embedded_url_formatter = SpeechFormatter::new(SpeechFormatterOptions {
        read_user_name: false,
        max_comment_length: 9,
        escape_bouyomi_tags: false,
        ..SpeechFormatterOptions::default()
    });
    assert_eq!(
        embedded_url_formatter.format_chat_message(&chat(
            "(https://example.com/path)と「www.example.com/path」"
        )),
        SpeechFormatDecision::Speak("(URL省略)と…".to_string())
    );

    let block_url_formatter = SpeechFormatter::new(SpeechFormatterOptions {
        read_user_name: false,
        max_comment_length: 1,
        replace_urls: false,
        block_urls: true,
        ..SpeechFormatterOptions::default()
    });
    assert_eq!(
        block_url_formatter.format_chat_message(&chat("https://example.com/path")),
        SpeechFormatDecision::Blocked(BlockedReason::BlockedUrl)
    );

    let block_formatter = SpeechFormatter::new(SpeechFormatterOptions {
        read_user_name: false,
        max_comment_length: 4,
        blocked_words: vec!["badword".to_string()],
        ..SpeechFormatterOptions::default()
    });
    assert_eq!(
        block_formatter.format_chat_message(&chat("safe prefix badword")),
        SpeechFormatDecision::Blocked(BlockedReason::BlockedWord)
    );
}

#[test]
fn formatter_still_blocks_empty_body_at_minimum_length() {
    let formatter = SpeechFormatter::new(SpeechFormatterOptions {
        read_user_name: true,
        max_comment_length: 1,
        ..SpeechFormatterOptions::default()
    });

    assert_eq!(
        formatter.format_chat_message(&chat("\u{0007}\n")),
        SpeechFormatDecision::Blocked(BlockedReason::EmptyAfterFormatting)
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
fn cancelled_in_flight_results_cannot_change_new_items_or_worker_ownership() {
    for operation in ["clear", "skip", "remove"] {
        for succeeds in [false, true] {
            let mut queue = SpeechQueueState::default();
            queue.pending.push_back(queued_item("old"));
            assert!(queue.claim_worker());
            let request = queue.begin_next_request().unwrap();
            assert!(queue.pending.is_empty());
            match operation {
                "clear" => queue.clear_pending(),
                "skip" => queue.skip_current(),
                "remove" => {
                    assert!(queue.remove_pending_item(&request.id));
                }
                _ => unreachable!(),
            }
            queue.pending.push_back(queued_item("new"));
            assert!(queue.in_flight.is_none());
            assert!(queue.is_processing);
            assert!(
                !queue.claim_worker(),
                "the old physical send still owns the worker"
            );
            if succeeds {
                assert!(!queue.complete_request(&request.id));
            } else {
                assert_eq!(
                    queue.fail_request(&request.id),
                    SpeechQueueFailureTransition::Ignored
                );
            }
            assert_eq!(queue.history.len(), 1);
            assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Skipped);
            let next = queue.begin_next_request().unwrap();
            assert_eq!(next.id, "new");
            assert!(queue.complete_request(&next.id));
            assert!(queue.pending.is_empty());
            assert!(queue.in_flight.is_none());
        }
    }
}

#[test]
fn control_barrier_prevents_pending_item_reservation() {
    let mut queue = SpeechQueueState::default();
    queue.pending.push_back(queued_item("pending"));
    queue.controls_in_progress = 1;

    assert!(queue.begin_next_request().is_none());
    assert_eq!(
        queue.pending.front().map(|item| item.id.as_str()),
        Some("pending")
    );
    assert!(queue.in_flight.is_none());

    queue.controls_in_progress = 0;
    assert_eq!(
        queue.begin_next_request().map(|request| request.id),
        Some("pending".to_string())
    );
}

#[test]
fn cancelling_the_last_control_restarts_a_worker_for_pending_speech() {
    let mut queue = SpeechQueueState {
        controls_in_progress: 2,
        ..SpeechQueueState::default()
    };
    queue.pending.push_back(queued_item("pending"));

    assert!(!queue.claim_worker());
    assert!(!queue.cancel_control_and_claim_worker());
    assert!(!queue.is_processing);
    assert!(queue.cancel_control_and_claim_worker());
    assert!(queue.is_processing);
    assert_eq!(
        queue.begin_next_request().map(|request| request.id),
        Some("pending".to_string())
    );
}

#[test]
fn cancelling_a_scheduled_retry_does_not_restore_it_or_lose_new_work() {
    let mut queue = SpeechQueueState::default();
    queue.pending.push_back(queued_item("old"));
    assert!(queue.claim_worker());
    let old = queue.begin_next_request().unwrap();
    assert_eq!(
        queue.fail_request(&old.id),
        SpeechQueueFailureTransition::RetryScheduled
    );
    queue.clear_pending();
    queue.pending.push_back(queued_item("new"));
    assert!(!queue.activate_scheduled_retry(&old.id));
    assert!(!queue.claim_worker());
    assert_eq!(queue.begin_next_request().unwrap().id, "new");
}

#[test]
fn overflow_only_drops_unsent_items_and_preserves_in_flight_completion() {
    for succeeds in [false, true] {
        let mut queue = SpeechQueueState::default();
        queue.pending.push_back(queued_item("sending"));
        let request = queue.begin_next_request().unwrap();
        for index in 0..DEFAULT_QUEUE_LIMIT - 1 {
            queue
                .pending
                .push_back(queued_item(&format!("pending-{index}")));
        }
        assert!(queue.make_pending_room());
        queue.pending.push_back(queued_item("newest"));
        assert_eq!(queue.pending.len() + 1, DEFAULT_QUEUE_LIMIT);
        assert_eq!(queue.history[0].id, "pending-0");
        assert_eq!(
            queue.in_flight.as_ref().unwrap().status,
            SpeechQueueItemStatus::Speaking
        );
        assert!(queue.begin_next_request().is_none());
        if succeeds {
            assert!(queue.complete_request(&request.id));
        } else {
            assert_eq!(
                queue.fail_request(&request.id),
                SpeechQueueFailureTransition::RetryScheduled
            );
            assert_eq!(queue.pending.front().unwrap().id, "sending");
        }
        assert!(!queue
            .history
            .iter()
            .any(|item| item.id == "sending" && item.status == SpeechQueueItemStatus::Skipped));
    }
}

#[cfg(feature = "app")]
#[test]
fn snapshots_include_in_flight_and_clear_preserves_terminal_states() {
    let mut queue = SpeechQueueState::default();
    queue.pending.push_back(queued_item("sending"));
    queue.pending.push_back(queued_item("waiting"));
    queue.begin_next_request().unwrap();
    let snapshot = queue_event_snapshot(&queue, None);
    assert_eq!(snapshot.queued_count, 2);
    assert_eq!(snapshot.phase, SpeechQueuePhase::Speaking);
    assert_eq!(snapshot.items[0].id, "sending");
    queue.clear_pending();
    let snapshot = queue_event_snapshot(&queue, None);
    assert_eq!(snapshot.queued_count, 0);
    assert!(snapshot
        .items
        .iter()
        .all(|item| item.status == SpeechQueueItemStatus::Skipped));
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
fn accepted_but_unconfirmed_item_is_not_automatically_resent() {
    let mut queue = SpeechQueueState::default();
    queue.pending.push_back(queued_item("speech-1"));

    let request = queue.begin_next_request().expect("submitted request");
    assert!(queue.fail_after_acceptance(&request.id, &SpeechFailure::unknown("unconfirmed".into())));

    assert!(queue.pending.is_empty());
    assert!(queue.in_flight.is_none());
    let item = queue.history.front().expect("unconfirmed history");
    assert_eq!(item.status, SpeechQueueItemStatus::Error);
    assert_eq!(item.retry_count, 1);
    assert_eq!(
        item.delivery_state,
        SpeechQueueDeliveryState::RetryExhausted
    );
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
