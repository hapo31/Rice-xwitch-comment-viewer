use super::*;
use crate::app_events::{SpeechQueueItemStatus, SpeechQueuePhase, SpeechQueueUpdatedEvent};
use crate::settings::{AppSettings, SpeechSettings};
use crate::speech::runtime::{SpeechAdapterFactory, SpeechRuntime};
use crate::speech::{
    queue_event_snapshot, FailureCode, SpeechAdapter, SpeechControl, SpeechFuture, SpeechHealth,
    SpeechQueueDeliveryState, SpeechQueueItem, SpeechRequest, SpeechResult,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tokio::sync::{oneshot, Notify};

enum Submission {
    Accept,
    Fail(SpeechFailure),
    Wait(oneshot::Receiver<()>),
    WaitResult(oneshot::Receiver<Result<(), SpeechFailure>>),
}
enum Completion {
    Complete,
    Unconfirmed(SpeechFailure),
    Wait(oneshot::Receiver<()>),
    WaitResult(oneshot::Receiver<SpeechPlaybackCompletion>),
}

#[derive(Default)]
struct FakeAdapter {
    submissions: Mutex<VecDeque<Submission>>,
    completions: Mutex<VecDeque<Completion>>,
    calls: Mutex<Vec<String>>,
    submission_started: Notify,
    completion_started: Notify,
    controls: Mutex<VecDeque<Result<(), SpeechFailure>>>,
}
impl FakeAdapter {
    fn control(&self, name: &str) -> SpeechFuture<'_, Result<(), SpeechFailure>> {
        self.calls.lock().unwrap().push(name.to_string());
        let result = self.controls.lock().unwrap().pop_front().unwrap_or(Ok(()));
        Box::pin(async move { result })
    }
}
impl SpeechAdapter for FakeAdapter {
    fn health_check(&self) -> SpeechFuture<'_, Result<SpeechHealth, SpeechFailure>> {
        self.calls.lock().unwrap().push("health".to_string());
        Box::pin(async { Ok(SpeechHealth::Connected) })
    }
    fn speak(
        &self,
        request: SpeechRequest,
    ) -> SpeechFuture<'_, Result<SpeechResult, SpeechFailure>> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("talk:{}", request.id));
        let action = self
            .submissions
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Submission::Accept);
        Box::pin(async move {
            self.submission_started.notify_one();
            match action {
                Submission::Accept => Ok(SpeechResult::Accepted),
                Submission::Fail(failure) => Err(failure),
                Submission::Wait(release) => {
                    release.await.expect("release submission");
                    Ok(SpeechResult::Accepted)
                }
                Submission::WaitResult(release) => {
                    release.await.expect("release submission")?;
                    Ok(SpeechResult::Accepted)
                }
            }
        })
    }
    fn pause(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>> {
        self.control("pause")
    }
    fn resume(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>> {
        self.control("resume")
    }
    fn skip(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>> {
        self.control("skip")
    }
    fn clear(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>> {
        self.control("clear")
    }
    fn wait_for_completion(&self) -> SpeechFuture<'_, SpeechPlaybackCompletion> {
        self.calls.lock().unwrap().push("completion".to_string());
        let action = self
            .completions
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Completion::Complete);
        Box::pin(async move {
            self.completion_started.notify_one();
            match action {
                Completion::Complete => SpeechPlaybackCompletion::Completed,
                Completion::Unconfirmed(failure) => SpeechPlaybackCompletion::Unconfirmed(failure),
                Completion::Wait(release) => {
                    release.await.expect("release completion");
                    SpeechPlaybackCompletion::Completed
                }
                Completion::WaitResult(release) => release.await.expect("release completion"),
            }
        })
    }
}

struct FakeFactory {
    adapter: Arc<FakeAdapter>,
    selections: AtomicUsize,
}
impl SpeechAdapterFactory for FakeFactory {
    fn select(
        &self,
        _: &SpeechSettings,
        _: SpeechDispatcher,
    ) -> Result<Arc<dyn SpeechAdapter>, SpeechFailure> {
        self.selections.fetch_add(1, Ordering::SeqCst);
        Ok(self.adapter.clone())
    }
}

struct FakeClock {
    state: Mutex<(Instant, Vec<Duration>)>,
}
impl Default for FakeClock {
    fn default() -> Self {
        Self {
            state: Mutex::new((Instant::now(), Vec::new())),
        }
    }
}
impl SpeechClock for FakeClock {
    fn now(&self) -> Instant {
        self.state.lock().unwrap().0
    }
    fn sleep(&self, delay: Duration) -> SpeechFuture<'_, ()> {
        Box::pin(async move {
            {
                let mut state = self.state.lock().unwrap();
                state.0 += delay;
                state.1.push(delay);
            }
            tokio::task::yield_now().await;
        })
    }
}

#[derive(Default)]
struct FakeEvents {
    snapshots: Mutex<Vec<SpeechQueueUpdatedEvent>>,
    health: Mutex<Vec<SpeechAdapterHealth>>,
    logs: Mutex<Vec<String>>,
}
impl SpeechQueueEvents for FakeEvents {
    fn snapshot(&self, queue: &SpeechQueueState, warning: Option<String>) {
        self.snapshots
            .lock()
            .unwrap()
            .push(queue_event_snapshot(queue, warning));
    }
    fn activity(&self, _: SpeechStatus, _: Option<String>) {}
    fn health(&self, health: SpeechAdapterHealth, _: Option<String>) {
        self.health.lock().unwrap().push(health);
    }
    fn log(&self, _: AppLogLevel, message: String) {
        self.logs.lock().unwrap().push(message);
    }
}

struct Harness {
    worker: Arc<SpeechQueueWorker>,
    runtime: SpeechRuntime,
    adapter: Arc<FakeAdapter>,
    factory: Arc<FakeFactory>,
    clock: Arc<FakeClock>,
    events: Arc<FakeEvents>,
}

mod scenarios;
fn queued(id: &str) -> SpeechQueueItem {
    SpeechQueueItem {
        id: id.to_string(),
        source_message_id: Some(format!("chat-{id}")),
        user_display_name: "viewer".to_string(),
        text: "test".to_string(),
        status: SpeechQueueItemStatus::Queued,
        retry_count: 0,
        delivery_state: SpeechQueueDeliveryState::Ready,
        outcome: None,
    }
}
fn harness(ids: &[&str], submissions: Vec<Submission>, completions: Vec<Completion>) -> Harness {
    let adapter = Arc::new(FakeAdapter {
        submissions: Mutex::new(submissions.into()),
        completions: Mutex::new(completions.into()),
        ..Default::default()
    });
    let factory = Arc::new(FakeFactory {
        adapter: adapter.clone(),
        selections: AtomicUsize::new(0),
    });
    let clock = Arc::new(FakeClock::default());
    let events = Arc::new(FakeEvents::default());
    let runtime = SpeechRuntime::new(factory.clone(), clock.clone());
    let selector = runtime.clone();
    let mut queue = SpeechQueueState::default();
    queue.pending.extend(ids.iter().map(|id| queued(id)));
    assert!(queue.claim_worker());
    let worker = Arc::new(SpeechQueueWorker {
        queue: Arc::new(Mutex::new(queue)),
        dispatcher: runtime.dispatcher(),
        select: Arc::new(move || selector.select(&AppSettings::default().speech)),
        clock: clock.clone(),
        events: events.clone(),
    });
    Harness {
        worker,
        runtime,
        adapter,
        factory,
        clock,
        events,
    }
}
fn failure(retryable: bool) -> SpeechFailure {
    SpeechFailure {
        code: FailureCode::ConnectionLost,
        status: SpeechStatus::Disconnected,
        retryable,
        user_message: "fake adapter未接続".to_string(),
        detail: "fake transport".to_string(),
    }
}

#[tokio::test]
async fn production_worker_keeps_typed_retry_outcome_and_correlates_warning_and_log() {
    use crate::speech::outcome::SpeechQueueOutcome;
    let mut first = failure(true);
    first.code = FailureCode::ConnectTimeout;
    let mut last = failure(true);
    last.code = FailureCode::ConnectionRefused;
    let h = harness(
        &["failed-item"],
        vec![Submission::Fail(first), Submission::Fail(last)],
        vec![],
    );
    h.worker.run().await;
    let snapshots = h.events.snapshots.lock().unwrap();
    let final_item = &snapshots.last().unwrap().items[0];
    assert!(matches!(
        final_item.outcome,
        Some(SpeechQueueOutcome::Error {
            reason_code: FailureCode::ConnectionRefused,
            ..
        })
    ));
    let warnings = snapshots
        .iter()
        .filter_map(|event| event.warning.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), 2);
    assert!(warnings
        .iter()
        .all(|message| message.contains("[failed-item]")));
    assert!(h
        .events
        .logs
        .lock()
        .unwrap()
        .iter()
        .all(|message| message.contains("[failed-item]")));
    assert!(snapshots.iter().any(|event| event
        .items
        .iter()
        .any(|item| item.status == SpeechQueueItemStatus::Queued && item.outcome.is_some())));
}

#[tokio::test]
async fn production_worker_preserves_uncertain_completion_without_resending() {
    use crate::speech::outcome::{RecoveryAction, SpeechQueueOutcome};
    let mut response = failure(false);
    response.code = FailureCode::ResponseTimeout;
    let h = harness(
        &["accepted-item"],
        vec![],
        vec![Completion::Unconfirmed(response)],
    );
    h.worker.run().await;
    assert_eq!(
        *h.adapter.calls.lock().unwrap(),
        ["talk:accepted-item", "completion"]
    );
    let queue = h.worker.queue.lock().unwrap();
    match queue.history.front().unwrap().outcome.as_ref().unwrap() {
        SpeechQueueOutcome::Error {
            reason_code,
            details,
        } => {
            assert_eq!(*reason_code, FailureCode::ResponseTimeout);
            assert_eq!(details.recovery_action, RecoveryAction::ConfirmDelivery);
            assert!(!details.retryable);
        }
        _ => panic!("error outcome"),
    }
    assert!(h
        .events
        .logs
        .lock()
        .unwrap()
        .iter()
        .all(|message| message.contains("[accepted-item]")));
}

#[tokio::test]
async fn production_worker_uses_a_fake_adapter_for_fifo_success() {
    let h = harness(&["first", "second"], vec![], vec![]);
    h.worker.run().await;
    assert_eq!(
        *h.adapter.calls.lock().unwrap(),
        ["talk:first", "completion", "talk:second", "completion"]
    );
    assert_eq!(h.factory.selections.load(Ordering::SeqCst), 2);
    let queue = h.worker.queue.lock().unwrap();
    assert!(!queue.is_processing);
    assert!(queue.pending.is_empty() && queue.in_flight.is_none());
    assert!(queue
        .history
        .iter()
        .all(|item| item.status == SpeechQueueItemStatus::Spoken));
    let snapshot = h.events.snapshots.lock().unwrap().last().unwrap().clone();
    assert_eq!(snapshot.queued_count, 0);
    assert_eq!(snapshot.items.len(), 2);
    assert_eq!(snapshot.phase, SpeechQueuePhase::Idle);
}

#[tokio::test]
async fn retry_uses_the_injected_clock_and_keeps_fifo() {
    let h = harness(
        &["first", "second"],
        vec![Submission::Fail(failure(true)), Submission::Accept],
        vec![],
    );
    let started = h.clock.now();
    h.worker.run().await;
    assert_eq!(
        *h.adapter.calls.lock().unwrap(),
        [
            "talk:first",
            "talk:first",
            "completion",
            "talk:second",
            "completion"
        ]
    );
    assert_eq!(h.clock.now() - started, RETRY_DELAY);
    assert_eq!(h.clock.state.lock().unwrap().1, [RETRY_DELAY]);
    assert_eq!(
        h.events.health.lock().unwrap().as_slice(),
        [SpeechAdapterHealth::Disconnected]
    );
    let queue = h.worker.queue.lock().unwrap();
    assert_eq!(
        queue
            .history
            .iter()
            .find(|item| item.id == "first")
            .unwrap()
            .retry_count,
        1
    );
    assert!(queue
        .history
        .iter()
        .all(|item| item.status == SpeechQueueItemStatus::Spoken));
}

#[tokio::test]
async fn retry_exhaustion_preserves_failed_history_and_processes_later_items() {
    let h = harness(
        &["first", "second"],
        vec![
            Submission::Fail(failure(true)),
            Submission::Fail(failure(true)),
        ],
        vec![],
    );
    h.worker.run().await;
    assert_eq!(
        *h.adapter.calls.lock().unwrap(),
        ["talk:first", "talk:first", "talk:second", "completion"]
    );
    let queue = h.worker.queue.lock().unwrap();
    assert_eq!(
        queue
            .history
            .iter()
            .find(|item| item.id == "first")
            .unwrap()
            .status,
        SpeechQueueItemStatus::Error
    );
    assert_eq!(
        queue
            .history
            .iter()
            .find(|item| item.id == "second")
            .unwrap()
            .status,
        SpeechQueueItemStatus::Spoken
    );
    assert_eq!(h.clock.state.lock().unwrap().1, [RETRY_DELAY]);
}

#[tokio::test]
async fn uncertain_submission_is_not_automatically_retried() {
    let h = harness(&["first"], vec![Submission::Fail(failure(false))], vec![]);
    h.worker.run().await;
    assert_eq!(*h.adapter.calls.lock().unwrap(), ["talk:first"]);
    assert!(h.clock.state.lock().unwrap().1.is_empty());
    assert_eq!(
        h.worker.queue.lock().unwrap().history[0].status,
        SpeechQueueItemStatus::Error
    );
}

#[tokio::test]
async fn accepted_but_unconfirmed_delivery_is_never_resent() {
    let h = harness(
        &["first", "second"],
        vec![],
        vec![Completion::Unconfirmed(failure(false))],
    );
    h.worker.run().await;
    assert_eq!(
        *h.adapter.calls.lock().unwrap(),
        ["talk:first", "completion", "talk:second", "completion"]
    );
    assert!(h.clock.state.lock().unwrap().1.is_empty());
    let queue = h.worker.queue.lock().unwrap();
    assert_eq!(
        queue
            .history
            .iter()
            .find(|item| item.id == "first")
            .unwrap()
            .status,
        SpeechQueueItemStatus::Error
    );
    assert_eq!(
        queue
            .history
            .iter()
            .find(|item| item.id == "first")
            .unwrap()
            .retry_count,
        1
    );
    assert_eq!(
        queue
            .history
            .iter()
            .find(|item| item.id == "second")
            .unwrap()
            .status,
        SpeechQueueItemStatus::Spoken
    );
}

#[tokio::test]
async fn delayed_submission_holds_ownership_and_the_common_dispatch_gate() {
    use futures_util::FutureExt;
    let (release, wait) = oneshot::channel();
    let h = harness(&["first"], vec![Submission::Wait(wait)], vec![]);
    let worker = h.worker.clone();
    let task = tokio::spawn(async move { worker.run().await });
    tokio::time::timeout(
        Duration::from_secs(2),
        h.adapter.submission_started.notified(),
    )
    .await
    .unwrap();
    {
        let mut queue = h.worker.queue.lock().unwrap();
        assert!(queue.in_flight.is_some());
        assert!(!queue.claim_worker());
    }
    let selected = h.runtime.select(&AppSettings::default().speech).unwrap();
    assert!(selected.lock().now_or_never().is_none());
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        h.worker.queue.lock().unwrap().history[0].status,
        SpeechQueueItemStatus::Spoken
    );
}

#[tokio::test]
async fn completion_wait_releases_the_dispatch_gate_for_control() {
    let (release, wait) = oneshot::channel();
    let h = harness(&["first", "second"], vec![], vec![Completion::Wait(wait)]);
    let worker = h.worker.clone();
    let task = tokio::spawn(async move { worker.run().await });
    tokio::time::timeout(
        Duration::from_secs(2),
        h.adapter.completion_started.notified(),
    )
    .await
    .unwrap();
    let selected = h.runtime.select(&AppSettings::default().speech).unwrap();
    let session = tokio::time::timeout(Duration::from_secs(2), selected.lock())
        .await
        .unwrap();
    session.control(SpeechControl::Pause).await.unwrap();
    h.worker.queue.lock().unwrap().paused = true;
    drop(session);
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    let queue = h.worker.queue.lock().unwrap();
    assert_eq!(queue.pending[0].id, "second");
    assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Spoken);
    assert_eq!(
        queue_event_snapshot(&queue, None).phase,
        SpeechQueuePhase::Paused
    );
    assert_eq!(
        *h.adapter.calls.lock().unwrap(),
        ["talk:first", "completion", "pause"]
    );
}

#[tokio::test]
async fn health_speak_and_all_controls_use_the_same_factory_boundary() {
    let h = harness(&["unused"], vec![], vec![]);
    for operation in 0..6 {
        let selected = h.runtime.select(&AppSettings::default().speech).unwrap();
        let session = selected.lock().await;
        match operation {
            0 => {
                assert!(matches!(
                    session.health_check().await.unwrap(),
                    SpeechHealth::Connected
                ));
            }
            1 => {
                session
                    .speak(SpeechRequest {
                        id: "test".to_string(),
                        source_message_id: None,
                        text: "test".to_string(),
                    })
                    .await
                    .unwrap();
            }
            n => {
                session
                    .control(
                        [
                            SpeechControl::Pause,
                            SpeechControl::Resume,
                            SpeechControl::Skip,
                            SpeechControl::Clear,
                        ][n - 2],
                    )
                    .await
                    .unwrap();
            }
        }
    }
    assert_eq!(h.factory.selections.load(Ordering::SeqCst), 6);
    assert_eq!(
        *h.adapter.calls.lock().unwrap(),
        ["health", "talk:test", "pause", "resume", "skip", "clear"]
    );
}

#[tokio::test]
async fn selection_failure_is_recorded_without_a_protocol_dependency() {
    let mut h = harness(&["first"], vec![], vec![]);
    Arc::get_mut(&mut h.worker).unwrap().select =
        Arc::new(|| Err(SpeechFailure::unknown("no adapter registered".to_string())));
    h.worker.run().await;
    assert!(h.adapter.calls.lock().unwrap().is_empty());
    let queue = h.worker.queue.lock().unwrap();
    assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Error);
    assert!(!queue.is_processing);
    assert_eq!(
        h.events.health.lock().unwrap().as_slice(),
        [SpeechAdapterHealth::Error]
    );
}
