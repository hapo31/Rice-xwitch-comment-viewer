use super::*;
use crate::speech::{
    enqueue_message, QueueEnqueueOutcome, SpeechFormatter, SpeechFormatterOptions,
    DEFAULT_HISTORY_LIMIT, DEFAULT_QUEUE_LIMIT,
};
use crate::twitch::{ChatMessage, Platform};
use std::collections::HashSet;

fn settings() -> SpeechSettings {
    let mut settings = AppSettings::default().speech;
    settings.repeat_suppression_seconds = 0;
    settings.read_user_name = false;
    settings
}
fn message(id: &str, user: &str) -> ChatMessage {
    ChatMessage {
        id: id.to_string(),
        platform: Platform::Twitch,
        channel_id: "channel".to_string(),
        channel_login: "channel".to_string(),
        user_id: user.to_string(),
        user_login: user.to_string(),
        user_display_name: user.to_string(),
        text: "test".to_string(),
        fragments: vec![],
        badges: vec![],
        received_at: chrono::DateTime::parse_from_rfc3339("2026-10-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        connection_generation: Some(1),
    }
}
fn invariants(queue: &SpeechQueueState) {
    assert!(queue.pending.len() + usize::from(queue.in_flight.is_some()) <= DEFAULT_QUEUE_LIMIT);
    assert!(queue.history.len() <= DEFAULT_HISTORY_LIMIT);
    let items = queue
        .pending
        .iter()
        .chain(queue.in_flight.iter())
        .chain(queue.history.iter())
        .collect::<Vec<_>>();
    let ids = items.iter().map(|item| &item.id).collect::<HashSet<_>>();
    assert_eq!(
        ids.len(),
        items.len(),
        "IDs must not occur in two lifecycle locations"
    );
    assert!(items.iter().all(|item| item.retry_count <= 1));
    assert!(queue
        .in_flight
        .as_ref()
        .is_none_or(|item| item.status == SpeechQueueItemStatus::Speaking));
    let snapshot = queue_event_snapshot(queue, None);
    assert_eq!(
        snapshot.queued_count,
        queue.pending.len() + usize::from(queue.in_flight.is_some())
    );
    assert_eq!(snapshot.items.len(), items.len());
    for item in items {
        let event = snapshot
            .items
            .iter()
            .find(|event| event.id == item.id)
            .unwrap();
        assert_eq!(event.status, item.status);
        assert_eq!(event.source_message_id, item.source_message_id);
    }
}
fn enqueue(h: &Harness, settings: &SpeechSettings, message: ChatMessage) -> QueueEnqueueOutcome {
    let formatter = SpeechFormatter::new(SpeechFormatterOptions::from(settings));
    let mut queue = h.worker.queue.lock().unwrap();
    let outcome = enqueue_message(&mut queue, settings, &formatter, message, h.clock.now());
    h.events.snapshot(&queue, outcome.warning.clone());
    invariants(&queue);
    outcome
}
async fn join(task: tokio::task::JoinHandle<()>) {
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("scheduler must settle")
        .unwrap();
}
async fn observed(signal: &Notify) {
    tokio::time::timeout(Duration::from_secs(2), signal.notified())
        .await
        .expect("expected boundary must be reached");
}

#[tokio::test]
async fn overflow_keeps_the_physical_send_and_drops_only_oldest_pending() {
    let (release, wait) = oneshot::channel();
    let h = harness(&["physical-send"], vec![Submission::Wait(wait)], vec![]);
    let worker = h.worker.clone();
    let task = tokio::spawn(async move { worker.run().await });
    observed(&h.adapter.submission_started).await;
    for index in 0..199 {
        let result = enqueue(&h, &settings(), message(&format!("chat-{index}"), "user"));
        assert!(!result.should_spawn);
        assert!(result.warning.is_none());
    }
    let result = enqueue(&h, &settings(), message("newest", "user"));
    assert!(result.warning.as_ref().unwrap().contains("上限"));
    {
        let queue = h.worker.queue.lock().unwrap();
        assert_eq!(queue.in_flight.as_ref().unwrap().id, "physical-send");
        assert_eq!(queue.pending.len(), 199);
        assert_eq!(
            queue.history[0].source_message_id.as_deref(),
            Some("chat-0")
        );
        assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Skipped);
        assert_eq!(
            queue.pending.front().unwrap().source_message_id.as_deref(),
            Some("chat-1")
        );
        assert_eq!(
            queue.pending.back().unwrap().source_message_id.as_deref(),
            Some("newest")
        );
    }
    let snapshot = h.events.snapshots.lock().unwrap().last().unwrap().clone();
    assert_eq!(snapshot.queued_count, 200);
    assert_eq!(snapshot.items.len(), 201);
    assert_eq!(snapshot.warning, result.warning);
    release.send(()).unwrap();
    join(task).await;
    let queue = h.worker.queue.lock().unwrap();
    invariants(&queue);
    assert!(queue.pending.is_empty() && queue.in_flight.is_none() && !queue.is_processing);
    assert_eq!(
        h.adapter
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.starts_with("talk:"))
            .count(),
        200
    );
}

#[tokio::test]
async fn repeat_suppression_uses_exact_clock_boundaries_and_independent_users() {
    for seconds in [0, 1, 2, 30] {
        let h = harness(&["seed"], vec![], vec![]);
        *h.worker.queue.lock().unwrap() = SpeechQueueState::default();
        let mut settings = settings();
        settings.repeat_suppression_seconds = seconds;
        assert!(enqueue(&h, &settings, message("a-first", "alice")).should_spawn);
        if seconds == 0 {
            assert!(enqueue(&h, &settings, message("a-repeat", "alice"))
                .warning
                .is_none());
            assert!(h.worker.queue.lock().unwrap().last_user_enqueue.is_empty());
            continue;
        }
        h.clock
            .sleep(Duration::from_secs(u64::from(seconds)) - Duration::from_millis(1))
            .await;
        let suppressed = enqueue(&h, &settings, message("a-suppressed", "alice"));
        assert!(suppressed.warning.as_ref().unwrap().contains("連投"));
        assert!(!suppressed.should_spawn);
        assert!(enqueue(&h, &settings, message("b-first", "bob"))
            .warning
            .is_none());
        h.clock.sleep(Duration::from_millis(1)).await;
        assert!(enqueue(&h, &settings, message("a-boundary", "alice"))
            .warning
            .is_none());
        assert!(enqueue(&h, &settings, message("b-suppressed", "bob"))
            .warning
            .is_some());
        let queue = h.worker.queue.lock().unwrap();
        assert_eq!(queue.pending.len(), 3);
        assert_eq!(queue.history.len(), 2);
        assert!(queue
            .history
            .iter()
            .all(|item| item.status == SpeechQueueItemStatus::Blocked));
        invariants(&queue);
    }
}

#[tokio::test]
async fn reconnect_does_not_resend_history_until_explicit_manual_retry() {
    let h = harness(
        &["failed"],
        vec![
            Submission::Fail(failure(true)),
            Submission::Fail(failure(true)),
            Submission::Fail(failure(true)),
            Submission::Accept,
        ],
        vec![],
    );
    h.worker.run().await;
    assert_eq!(
        h.adapter
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.starts_with("talk:"))
            .count(),
        2
    );
    let selected = h.runtime.select(&settings()).unwrap();
    let session = selected.lock().await;
    assert!(matches!(
        session.health_check().await.unwrap(),
        SpeechHealth::Connected
    ));
    drop(session);
    {
        let mut queue = h.worker.queue.lock().unwrap();
        assert_eq!(
            queue_event_snapshot(&queue, None).phase,
            SpeechQueuePhase::Error
        );
        assert!(!queue.claim_worker());
        assert!(queue.retry_exhausted_item("failed"));
        assert_eq!(queue.pending[0].retry_count, 0);
        assert!(queue.claim_worker());
    }
    h.worker.run().await;
    let queue = h.worker.queue.lock().unwrap();
    assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Spoken);
    assert_eq!(queue.history[0].retry_count, 1);
    assert_eq!(h.clock.state.lock().unwrap().1, [RETRY_DELAY, RETRY_DELAY]);
    invariants(&queue);
}

#[tokio::test]
async fn manual_retry_refuses_a_full_queue_without_losing_error_history() {
    for in_flight in [false, true] {
        let h = harness(&["failed"], vec![Submission::Fail(failure(false))], vec![]);
        h.worker.run().await;
        let mut queue = h.worker.queue.lock().unwrap();
        queue
            .pending
            .extend((0..200).map(|index| queued(&format!("pending-{index}"))));
        if in_flight {
            queue.begin_next_request().unwrap();
        }
        assert!(
            !queue.retry_exhausted_item("failed"),
            "manual retry must obey the same 200 item limit"
        );
        assert!(queue.history.iter().any(|item| item.id == "failed"));
        invariants(&queue);
        queue.pending.pop_back();
        assert!(queue.retry_exhausted_item("failed"));
        assert!(!queue.history.iter().any(|item| item.id == "failed"));
        invariants(&queue);
    }
}

#[tokio::test]
async fn cancellations_and_late_outcomes_never_resurrect_items_or_strand_new_work() {
    for operation in ["clear", "skip", "remove"] {
        for outcome in ["complete", "submission-error", "completion-error"] {
            let (submitted, submission_wait) = oneshot::channel();
            let (completed, completion_wait) = oneshot::channel();
            let h = harness(
                &["old"],
                vec![Submission::WaitResult(submission_wait)],
                vec![Completion::WaitResult(completion_wait)],
            );
            let worker = h.worker.clone();
            let task = tokio::spawn(async move { worker.run().await });
            observed(&h.adapter.submission_started).await;
            if operation == "remove" {
                assert!(h.worker.queue.lock().unwrap().remove_pending_item("old"));
            } else {
                // Native controls wait for the physical send, but begin their
                // local barrier first, so its late result cannot overtake them.
                h.worker.queue.lock().unwrap().begin_control();
            }
            if outcome == "submission-error" {
                h.adapter.completions.lock().unwrap().clear();
                submitted.send(Err(failure(true))).unwrap();
                apply_cancellation(&h, operation).await;
                assert!(!enqueue(&h, &settings(), message("new", "user")).should_spawn);
                drop(completed);
            } else {
                submitted.send(Ok(())).unwrap();
                observed(&h.adapter.completion_started).await;
                apply_cancellation(&h, operation).await;
                assert!(!enqueue(&h, &settings(), message("new", "user")).should_spawn);
                let result = if outcome == "complete" {
                    SpeechPlaybackCompletion::Completed
                } else {
                    SpeechPlaybackCompletion::Unconfirmed(failure(false))
                };
                completed.send(result).unwrap();
            }
            join(task).await;
            let queue = h.worker.queue.lock().unwrap();
            invariants(&queue);
            assert_eq!(
                queue
                    .history
                    .iter()
                    .find(|item| item.id == "old")
                    .unwrap()
                    .status,
                SpeechQueueItemStatus::Skipped
            );
            assert_eq!(
                queue
                    .history
                    .iter()
                    .find(|item| item.source_message_id.as_deref() == Some("new"))
                    .unwrap()
                    .status,
                SpeechQueueItemStatus::Spoken
            );
            assert!(!queue.is_processing);
            assert!(queue.pending.is_empty() && queue.in_flight.is_none());
            assert!(
                !h.clock.state.lock().unwrap().1.contains(&RETRY_DELAY),
                "late failure cannot consume retry budget"
            );
            assert_eq!(
                h.adapter
                    .calls
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|call| call.as_str() == "talk:old")
                    .count(),
                1
            );
        }
    }
}

async fn apply_cancellation(h: &Harness, operation: &str) {
    if operation != "remove" {
        let command = if operation == "clear" {
            SpeechControl::Clear
        } else {
            SpeechControl::Skip
        };
        let selected = h.runtime.select(&settings()).unwrap();
        let session = selected.lock().await;
        session.control(command).await.unwrap();
        assert!(!h.worker.queue.lock().unwrap().apply_control(command));
    }
    let queue = h.worker.queue.lock().unwrap();
    assert!(queue.is_processing);
    assert!(queue.in_flight.is_none());
    assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Skipped);
}

#[tokio::test]
async fn controls_after_completion_do_not_relabel_completed_history() {
    for operation in ["clear", "skip", "remove"] {
        let h = harness(&["completed"], vec![], vec![]);
        h.worker.run().await;
        {
            let mut queue = h.worker.queue.lock().unwrap();
            queue.paused = true;
        }
        assert!(!enqueue(&h, &settings(), message("new", "user")).should_spawn);
        let mut queue = h.worker.queue.lock().unwrap();
        let pending_id = queue.pending[0].id.clone();
        match operation {
            "clear" => {
                queue.begin_control();
                queue.apply_control(SpeechControl::Clear);
            }
            "skip" => {
                queue.begin_control();
                queue.apply_control(SpeechControl::Skip);
            }
            "remove" => {
                queue.remove_pending_item(&pending_id);
                assert!(!queue.remove_pending_item("completed"));
            }
            _ => unreachable!(),
        }
        assert_eq!(
            queue
                .history
                .iter()
                .find(|item| item.id == "completed")
                .unwrap()
                .status,
            SpeechQueueItemStatus::Spoken
        );
        assert_eq!(
            queue
                .history
                .iter()
                .find(|item| item.id == pending_id)
                .unwrap()
                .status,
            SpeechQueueItemStatus::Skipped
        );
        invariants(&queue);
    }
}

#[tokio::test]
async fn pause_before_reservation_prevents_talk_until_explicit_resume() {
    let h = harness(&["first"], vec![], vec![]);
    let selected = h.runtime.select(&settings()).unwrap();
    let session = selected.lock().await;
    let worker = h.worker.clone();
    let task = tokio::spawn(async move { worker.run().await });
    {
        h.worker.queue.lock().unwrap().begin_control();
        session.control(SpeechControl::Pause).await.unwrap();
        assert!(!h
            .worker
            .queue
            .lock()
            .unwrap()
            .apply_control(SpeechControl::Pause));
    }
    drop(session);
    join(task).await;
    assert_eq!(*h.adapter.calls.lock().unwrap(), ["pause"]);
    {
        let queue = h.worker.queue.lock().unwrap();
        assert_eq!(queue.pending[0].id, "first");
        assert!(!queue.is_processing);
        assert_eq!(
            queue_event_snapshot(&queue, None).phase,
            SpeechQueuePhase::Paused
        );
    }
    h.worker.queue.lock().unwrap().begin_control();
    let session = selected.lock().await;
    session.control(SpeechControl::Resume).await.unwrap();
    assert!(h
        .worker
        .queue
        .lock()
        .unwrap()
        .apply_control(SpeechControl::Resume));
    drop(session);
    h.worker.run().await;
    assert_eq!(
        *h.adapter.calls.lock().unwrap(),
        ["pause", "resume", "talk:first", "completion"]
    );
    invariants(&h.worker.queue.lock().unwrap());
}

#[tokio::test]
async fn pause_during_send_is_ordered_before_completion_and_later_requests() {
    let (submit, submission_wait) = oneshot::channel();
    let (complete, completion_wait) = oneshot::channel();
    let h = Arc::new(harness(
        &["first", "second"],
        vec![Submission::Wait(submission_wait)],
        vec![Completion::Wait(completion_wait)],
    ));
    let worker = h.worker.clone();
    let task = tokio::spawn(async move { worker.run().await });
    observed(&h.adapter.submission_started).await;
    h.worker.queue.lock().unwrap().begin_control();
    let control_h = h.clone();
    let pause = tokio::spawn(async move {
        let selected = control_h.runtime.select(&settings()).unwrap();
        let session = selected.lock().await;
        session.control(SpeechControl::Pause).await.unwrap();
        assert!(!control_h
            .worker
            .queue
            .lock()
            .unwrap()
            .apply_control(SpeechControl::Pause));
    });
    submit.send(()).unwrap();
    observed(&h.adapter.completion_started).await;
    join(pause).await;
    complete.send(()).unwrap();
    join(task).await;
    {
        let queue = h.worker.queue.lock().unwrap();
        assert_eq!(queue.pending[0].id, "second");
        assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Spoken);
        assert_eq!(
            queue_event_snapshot(&queue, None).phase,
            SpeechQueuePhase::Paused
        );
        assert_eq!(queue.controls_in_progress, 0);
    }
    h.worker.queue.lock().unwrap().begin_control();
    let selected = h.runtime.select(&settings()).unwrap();
    let session = selected.lock().await;
    session.control(SpeechControl::Resume).await.unwrap();
    assert!(h
        .worker
        .queue
        .lock()
        .unwrap()
        .apply_control(SpeechControl::Resume));
    drop(session);
    h.worker.run().await;
    let calls = h.adapter.calls.lock().unwrap();
    let first = calls.iter().position(|call| call == "talk:first").unwrap();
    let pause = calls.iter().position(|call| call == "pause").unwrap();
    let resume = calls.iter().position(|call| call == "resume").unwrap();
    let second = calls.iter().position(|call| call == "talk:second").unwrap();
    assert!(first < pause && pause < resume && resume < second);
    invariants(&h.worker.queue.lock().unwrap());
}

#[tokio::test]
async fn failed_last_control_releases_its_barrier_and_restarts_pending_work() {
    let h = harness(&["first"], vec![], vec![]);
    h.adapter
        .controls
        .lock()
        .unwrap()
        .push_back(Err(failure(false)));
    {
        let mut queue = h.worker.queue.lock().unwrap();
        queue.is_processing = false;
        queue.begin_control();
        assert!(!queue.claim_worker());
    }
    let selected = h.runtime.select(&settings()).unwrap();
    let session = selected.lock().await;
    assert!(session.control(SpeechControl::Pause).await.is_err());
    {
        let mut queue = h.worker.queue.lock().unwrap();
        assert!(queue.cancel_control_and_claim_worker());
        assert_eq!(queue.controls_in_progress, 0);
        assert!(!queue.paused);
        assert_eq!(queue.pending[0].id, "first");
    }
    drop(session);
    h.worker.run().await;
    assert_eq!(
        h.worker.queue.lock().unwrap().history[0].status,
        SpeechQueueItemStatus::Spoken
    );
    invariants(&h.worker.queue.lock().unwrap());
}

struct ManualClock {
    now: Mutex<Instant>,
    waiting: Mutex<Vec<(Instant, oneshot::Sender<()>)>>,
    registered: Notify,
}
impl Default for ManualClock {
    fn default() -> Self {
        Self {
            now: Mutex::new(Instant::now()),
            waiting: Mutex::new(vec![]),
            registered: Notify::new(),
        }
    }
}
impl SpeechClock for ManualClock {
    fn now(&self) -> Instant {
        *self.now.lock().unwrap()
    }
    fn sleep(&self, delay: Duration) -> SpeechFuture<'_, ()> {
        Box::pin(async move {
            let (release, wait) = oneshot::channel();
            self.waiting
                .lock()
                .unwrap()
                .push((self.now() + delay, release));
            self.registered.notify_one();
            wait.await.expect("advance virtual clock");
        })
    }
}
impl ManualClock {
    fn advance(&self, delta: Duration) {
        *self.now.lock().unwrap() += delta;
        let now = self.now();
        let mut waiting = self.waiting.lock().unwrap();
        for index in (0..waiting.len()).rev() {
            if waiting[index].0 <= now {
                let (_, release) = waiting.remove(index);
                let _ = release.send(());
            }
        }
    }
}

#[tokio::test]
async fn retry_cannot_start_before_its_virtual_deadline() {
    let mut h = harness(
        &["first"],
        vec![Submission::Fail(failure(true)), Submission::Accept],
        vec![],
    );
    let clock = Arc::new(ManualClock::default());
    Arc::get_mut(&mut h.worker).unwrap().clock = clock.clone();
    let worker = h.worker.clone();
    let task = tokio::spawn(async move { worker.run().await });
    observed(&clock.registered).await;
    {
        let mut queue = h.worker.queue.lock().unwrap();
        assert!(!queue.claim_worker());
        assert_eq!(
            queue.pending[0].delivery_state,
            SpeechQueueDeliveryState::RetryScheduled
        );
        assert!(queue.in_flight.is_none());
        invariants(&queue);
    }
    clock.advance(Duration::from_millis(699));
    tokio::task::yield_now().await;
    assert_eq!(*h.adapter.calls.lock().unwrap(), ["talk:first"]);
    clock.advance(Duration::from_millis(1));
    join(task).await;
    assert_eq!(
        *h.adapter.calls.lock().unwrap(),
        ["talk:first", "talk:first", "completion"]
    );
    invariants(&h.worker.queue.lock().unwrap());
}

#[tokio::test]
async fn clear_during_retry_delay_does_not_activate_the_cancelled_request() {
    let mut h = harness(
        &["old"],
        vec![Submission::Fail(failure(true)), Submission::Accept],
        vec![],
    );
    let clock = Arc::new(ManualClock::default());
    Arc::get_mut(&mut h.worker).unwrap().clock = clock.clone();
    let worker = h.worker.clone();
    let task = tokio::spawn(async move { worker.run().await });
    observed(&clock.registered).await;
    h.worker.queue.lock().unwrap().begin_control();
    let selected = h.runtime.select(&settings()).unwrap();
    let session = selected.lock().await;
    session.control(SpeechControl::Clear).await.unwrap();
    assert!(!h
        .worker
        .queue
        .lock()
        .unwrap()
        .apply_control(SpeechControl::Clear));
    drop(session);
    assert!(!enqueue(&h, &settings(), message("new", "user")).should_spawn);
    clock.advance(RETRY_DELAY);
    join(task).await;
    let queue = h.worker.queue.lock().unwrap();
    assert_eq!(
        queue
            .history
            .iter()
            .find(|item| item.id == "old")
            .unwrap()
            .status,
        SpeechQueueItemStatus::Skipped
    );
    assert_eq!(
        h.adapter
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.as_str() == "talk:old")
            .count(),
        1
    );
    assert_eq!(
        queue
            .history
            .iter()
            .find(|item| item.source_message_id.as_deref() == Some("new"))
            .unwrap()
            .status,
        SpeechQueueItemStatus::Spoken
    );
    invariants(&queue);
}

#[tokio::test]
async fn simultaneous_enqueue_keeps_one_worker_and_covers_the_empty_exit_boundary() {
    let (release, wait) = oneshot::channel();
    let h = Arc::new(harness(&["seed"], vec![Submission::Wait(wait)], vec![]));
    *h.worker.queue.lock().unwrap() = SpeechQueueState::default();
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let starts = Arc::new(AtomicUsize::new(0));
    let workers = Arc::new(Mutex::new(Vec::<tokio::task::JoinHandle<()>>::new()));
    let mut release = Some(release);
    for epoch in 0..2 {
        let barrier = Arc::new(tokio::sync::Barrier::new(21));
        let mut producers = vec![];
        for producer in 0..20 {
            let h = h.clone();
            let barrier = barrier.clone();
            let workers = workers.clone();
            let active = active.clone();
            let peak = peak.clone();
            let starts = starts.clone();
            producers.push(tokio::spawn(async move {
                barrier.wait().await;
                let outcome = enqueue(
                    &h,
                    &settings(),
                    message(&format!("e{epoch}-p{producer}"), "user"),
                );
                if outcome.should_spawn {
                    starts.fetch_add(1, Ordering::SeqCst);
                    let worker = h.worker.clone();
                    workers.lock().unwrap().push(tokio::spawn(async move {
                        let live = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(live, Ordering::SeqCst);
                        worker.run().await;
                        active.fetch_sub(1, Ordering::SeqCst);
                    }));
                }
            }));
        }
        barrier.wait().await;
        for producer in producers {
            join(producer).await;
        }
        if epoch == 0 {
            observed(&h.adapter.submission_started).await;
            assert_eq!(starts.load(Ordering::SeqCst), 1);
            assert_eq!(h.worker.queue.lock().unwrap().pending.len(), 19);
            release.take().unwrap().send(()).unwrap();
        }
        let tasks = std::mem::take(&mut *workers.lock().unwrap());
        for task in tasks {
            join(task).await;
        }
        let queue = h.worker.queue.lock().unwrap();
        invariants(&queue);
        assert!(queue.pending.is_empty() && queue.in_flight.is_none() && !queue.is_processing);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }
    assert_eq!(peak.load(Ordering::SeqCst), 1);
    assert!(starts.load(Ordering::SeqCst) >= 2);
    let calls = h.adapter.calls.lock().unwrap();
    let talk = calls
        .iter()
        .filter(|call| call.starts_with("talk:"))
        .collect::<Vec<_>>();
    assert_eq!(talk.len(), 40);
    assert_eq!(talk.into_iter().collect::<HashSet<_>>().len(), 40);
}

#[test]
fn seeded_operation_sequences_preserve_lifecycle_invariants() {
    for seed in [1_u64, 42, 0x7192] {
        let h = harness(&["seed"], vec![], vec![]);
        *h.worker.queue.lock().unwrap() = SpeechQueueState::default();
        let mut random = seed;
        for index in 0..2_048 {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let operation = (random >> 32) % 10;
            if operation <= 2 {
                enqueue(&h, &settings(), message(&format!("{seed}-{index}"), "user"));
            } else {
                let mut queue = h.worker.queue.lock().unwrap();
                match operation {
                    3 => {
                        queue.begin_control();
                        queue.apply_control(SpeechControl::Pause);
                    }
                    4 => {
                        queue.begin_control();
                        queue.apply_control(SpeechControl::Resume);
                    }
                    5 => {
                        queue.begin_control();
                        queue.apply_control(SpeechControl::Skip);
                    }
                    6 => {
                        queue.begin_control();
                        queue.apply_control(SpeechControl::Clear);
                    }
                    7 => {
                        if let Some(id) = queue.pending.front().map(|item| item.id.clone()) {
                            queue.remove_pending_item(&id);
                        }
                    }
                    8 => {
                        if let Some(id) = queue
                            .history
                            .iter()
                            .find(|item| item.status == SpeechQueueItemStatus::Error)
                            .map(|item| item.id.clone())
                        {
                            queue.retry_exhausted_item(&id);
                            queue.claim_worker();
                        }
                    }
                    9 => {
                        queue.dismiss_history();
                    }
                    _ => unreachable!(),
                }
            }
            let mut queue = h.worker.queue.lock().unwrap();
            if queue.is_processing {
                assert!(!queue.claim_worker());
                if let Some(request) = queue.reserve_next_request_after_dispatch_lock() {
                    if random & 1 == 0 {
                        queue.complete_request(&request.id);
                    } else if queue.fail_request_with_retry(&request.id, true)
                        == SpeechQueueFailureTransition::RetryScheduled
                    {
                        queue.activate_scheduled_retry(&request.id);
                    }
                } else if queue.paused || queue.pending.is_empty() {
                    queue.is_processing = false;
                }
            }
            invariants(&queue);
            if !queue.paused && queue.controls_in_progress == 0 && queue.has_auto_processable_item()
            {
                assert!(
                    queue.is_processing,
                    "processable work must retain a worker owner at seed={seed}, operation={index}"
                );
            }
        }
    }
}

#[test]
fn formatter_blocks_and_disabled_auto_speech_keep_snapshots_consistent() {
    for kind in ["user", "word", "url", "empty", "auto-off"] {
        let h = harness(&["seed"], vec![], vec![]);
        *h.worker.queue.lock().unwrap() = SpeechQueueState::default();
        let mut settings = settings();
        let mut message = message("message", "alice");
        match kind {
            "user" => settings.blocked_users = vec!["alice".to_string()],
            "word" => settings.blocked_words = vec!["test".to_string()],
            "url" => {
                settings.url_handling = crate::settings::UrlHandling::Block;
                message.text = "https://example.com".to_string();
            }
            "empty" => message.text = "\u{0}".to_string(),
            "auto-off" => settings.auto_speak = false,
            _ => unreachable!(),
        }
        let outcome = enqueue(&h, &settings, message);
        assert!(!outcome.should_spawn && !outcome.should_schedule_cleanup);
        let queue = h.worker.queue.lock().unwrap();
        assert_eq!(
            queue_event_snapshot(&queue, outcome.warning.clone()).queued_count,
            0
        );
        if kind == "auto-off" {
            assert!(queue.history.is_empty());
            assert!(outcome.warning.is_none());
        } else {
            assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Blocked);
            assert!(outcome.warning.is_some());
        }
    }
}

#[tokio::test]
async fn overlapping_controls_release_only_the_last_barrier() {
    let h = harness(&["first"], vec![], vec![]);
    h.adapter
        .controls
        .lock()
        .unwrap()
        .extend([Err(failure(false)), Ok(())]);
    {
        let mut queue = h.worker.queue.lock().unwrap();
        queue.is_processing = false;
        queue.begin_control();
        queue.begin_control();
        assert!(!queue.claim_worker());
    }
    let selected = h.runtime.select(&settings()).unwrap();
    let session = selected.lock().await;
    assert!(session.control(SpeechControl::Pause).await.is_err());
    {
        let mut queue = h.worker.queue.lock().unwrap();
        assert!(!queue.cancel_control_and_claim_worker());
        assert_eq!(queue.controls_in_progress, 1);
        assert!(!queue.is_processing && !queue.paused);
    }
    drop(session);
    let session = selected.lock().await;
    session.control(SpeechControl::Resume).await.unwrap();
    assert!(h
        .worker
        .queue
        .lock()
        .unwrap()
        .apply_control(SpeechControl::Resume));
    drop(session);
    h.worker.run().await;
    assert_eq!(
        *h.adapter.calls.lock().unwrap(),
        ["pause", "resume", "talk:first", "completion"]
    );
    invariants(&h.worker.queue.lock().unwrap());
}

#[tokio::test]
async fn native_thread_enqueue_claims_only_one_worker_owner() {
    let h = Arc::new(harness(&["seed"], vec![], vec![]));
    *h.worker.queue.lock().unwrap() = SpeechQueueState::default();
    let barrier = Arc::new(std::sync::Barrier::new(21));
    let claims = Arc::new(AtomicUsize::new(0));
    let threads = (0..20)
        .map(|index| {
            let h = h.clone();
            let barrier = barrier.clone();
            let claims = claims.clone();
            std::thread::spawn(move || {
                barrier.wait();
                if enqueue(&h, &settings(), message(&format!("thread-{index}"), "user"))
                    .should_spawn
                {
                    claims.fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(claims.load(Ordering::SeqCst), 1);
    assert_eq!(h.worker.queue.lock().unwrap().pending.len(), 20);
    h.worker.run().await;
    let queue = h.worker.queue.lock().unwrap();
    assert!(queue.pending.is_empty() && queue.in_flight.is_none() && !queue.is_processing);
    assert_eq!(queue.history.len(), 20);
    invariants(&queue);
}
