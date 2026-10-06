use super::*;

struct TwoDestinations {
    a: Arc<FakeAdapter>,
    b: Arc<FakeAdapter>,
}
impl SpeechAdapterFactory for TwoDestinations {
    fn select(
        &self,
        settings: &SpeechSettings,
        _: SpeechDispatcher,
    ) -> Result<Arc<dyn SpeechAdapter>, SpeechFailure> {
        Ok(if settings.bouyomi_port == 50001 {
            self.a.clone()
        } else {
            self.b.clone()
        })
    }
}

struct DestinationHarness {
    runtime: SpeechRuntime,
    settings: Arc<Mutex<SpeechSettings>>,
    worker: Arc<SpeechQueueWorker>,
    a: Arc<FakeAdapter>,
    b: Arc<FakeAdapter>,
}
impl DestinationHarness {
    fn new(submissions: Vec<Submission>, completion: oneshot::Receiver<()>) -> Self {
        let a = Arc::new(FakeAdapter {
            submissions: Mutex::new(submissions.into()),
            completions: Mutex::new(vec![Completion::Wait(completion)].into()),
            ..Default::default()
        });
        let b = Arc::new(FakeAdapter::default());
        let runtime = SpeechRuntime::new(
            Arc::new(TwoDestinations {
                a: a.clone(),
                b: b.clone(),
            }),
            Arc::new(FakeClock::default()),
        );
        let settings = Arc::new(Mutex::new(AppSettings::default().speech));
        let configured = settings.clone();
        let selector = runtime.clone();
        let mut queue = SpeechQueueState::default();
        queue.pending.extend([queued("first"), queued("second")]);
        assert!(queue.claim_worker());
        let worker = Arc::new(SpeechQueueWorker {
            queue: Arc::new(Mutex::new(queue)),
            dispatcher: runtime.dispatcher(),
            select: Arc::new(move || selector.select(&configured.lock().unwrap())),
            clock: Arc::new(FakeClock::default()),
            events: Arc::new(FakeEvents::default()),
        });
        Self {
            runtime,
            settings,
            worker,
            a,
            b,
        }
    }
    fn save_b(&self) {
        self.settings.lock().unwrap().bouyomi_port = 50002;
        self.runtime.destination_policy().revoke();
    }
    async fn control(&self, command: SpeechControl) -> Result<(), SpeechFailure> {
        self.worker.queue.lock().unwrap().begin_control();
        let session = self
            .runtime
            .lock_control(|| self.runtime.select(&self.settings.lock().unwrap()))
            .await?;
        match session.control(command).await {
            Ok(()) => {
                self.worker.queue.lock().unwrap().apply_control(command);
                Ok(())
            }
            Err(error) => {
                self.worker
                    .queue
                    .lock()
                    .unwrap()
                    .cancel_control_and_claim_worker();
                Err(error)
            }
        }
    }
    fn spawn(&self) -> tokio::task::JoinHandle<()> {
        let worker = self.worker.clone();
        tokio::spawn(async move { worker.run().await })
    }
}

async fn observed(signal: &Notify) {
    tokio::time::timeout(Duration::from_secs(2), signal.notified())
        .await
        .unwrap();
}
async fn joined(task: tokio::task::JoinHandle<()>) {
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn configured_b_does_not_redirect_active_a_controls_or_completion() {
    for command in [
        SpeechControl::Pause,
        SpeechControl::Resume,
        SpeechControl::Skip,
        SpeechControl::Clear,
    ] {
        let (release, completion) = oneshot::channel();
        let h = DestinationHarness::new(vec![], completion);
        let task = h.spawn();
        observed(&h.a.completion_started).await;
        h.save_b();
        if command == SpeechControl::Resume {
            h.control(SpeechControl::Pause).await.unwrap();
        }
        h.control(command).await.unwrap();
        assert!(
            h.b.calls.lock().unwrap().is_empty(),
            "B must not receive A's control"
        );
        {
            let queue = h.worker.queue.lock().unwrap();
            match command {
                SpeechControl::Pause => assert!(queue.paused),
                SpeechControl::Resume => assert!(!queue.paused),
                SpeechControl::Skip => {
                    assert_eq!(queue.history[0].status, SpeechQueueItemStatus::Skipped)
                }
                SpeechControl::Clear => {
                    assert!(queue.in_flight.is_none() && queue.pending.is_empty())
                }
            }
        }
        if command == SpeechControl::Pause {
            h.control(SpeechControl::Resume).await.unwrap();
        }
        release.send(()).unwrap();
        joined(task).await;
        let expected = match command {
            SpeechControl::Pause | SpeechControl::Resume => {
                vec!["talk:first", "completion", "pause", "resume"]
            }
            SpeechControl::Skip => vec!["talk:first", "completion", "skip"],
            SpeechControl::Clear => vec!["talk:first", "completion", "clear"],
        };
        assert_eq!(*h.a.calls.lock().unwrap(), expected);
        let expected_b = if command == SpeechControl::Clear {
            vec![]
        } else {
            vec!["talk:second", "completion"]
        };
        assert_eq!(*h.b.calls.lock().unwrap(), expected_b);
        // After the playback owner is released, controls use current configuration.
        h.control(SpeechControl::Clear).await.unwrap();
        assert_eq!(h.b.calls.lock().unwrap().last().unwrap(), "clear");
    }
}

#[tokio::test]
async fn control_waiting_on_talk_selects_the_new_active_session_after_the_gate() {
    use futures_util::FutureExt;
    let (accept, submission) = oneshot::channel();
    let (release, completion) = oneshot::channel();
    let h = DestinationHarness::new(vec![Submission::Wait(submission)], completion);
    let task = h.spawn();
    observed(&h.a.submission_started).await;
    h.save_b();
    let control = h.control(SpeechControl::Skip);
    tokio::pin!(control);
    assert!(control.as_mut().now_or_never().is_none());
    accept.send(()).unwrap();
    control.await.unwrap();
    assert!(h.a.calls.lock().unwrap().contains(&"skip".to_string()));
    assert!(h.b.calls.lock().unwrap().is_empty());
    release.send(()).unwrap();
    joined(task).await;
    assert_eq!(*h.b.calls.lock().unwrap(), ["talk:second", "completion"]);
}

#[tokio::test]
async fn failed_active_control_does_not_fall_back_to_b_or_change_local_queue() {
    let (release, completion) = oneshot::channel();
    let h = DestinationHarness::new(vec![], completion);
    let task = h.spawn();
    observed(&h.a.completion_started).await;
    h.save_b();
    h.a.controls.lock().unwrap().push_back(Err(failure(false)));
    assert!(h.control(SpeechControl::Skip).await.is_err());
    assert!(h.b.calls.lock().unwrap().is_empty());
    {
        let queue = h.worker.queue.lock().unwrap();
        assert_eq!(queue.in_flight.as_ref().unwrap().id, "first");
        assert!(queue.history.is_empty());
        assert_eq!(queue.controls_in_progress, 0);
    }
    release.send(()).unwrap();
    joined(task).await;
}
