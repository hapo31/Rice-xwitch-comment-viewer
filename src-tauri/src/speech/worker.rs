use super::queue::{SpeechQueueFailureTransition, SpeechQueueState, RETRY_DELAY};
use super::runtime::{SelectedSpeechAdapter, SpeechClock, SpeechDispatcher};
use super::{SpeechPlaybackCompletion, SpeechStatus};
use crate::app_events::{AppLogLevel, SpeechAdapterHealth};
use crate::speech::SpeechFailure;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Kept separate from the scheduler so no Tauri handle, TCP, or real clock is
/// needed by tests. Snapshots are emitted while holding the queue mutex, as in
/// production, preserving the revision/snapshot atomicity contract.
pub(crate) trait SpeechQueueEvents: Send + Sync {
    fn snapshot(&self, queue: &SpeechQueueState, warning: Option<String>);
    fn activity(&self, status: SpeechStatus, message: Option<String>);
    fn health(&self, health: SpeechAdapterHealth, message: Option<String>);
    fn log(&self, level: AppLogLevel, message: String);
}

pub(crate) struct SpeechQueueWorker {
    pub queue: Arc<Mutex<SpeechQueueState>>,
    pub dispatcher: SpeechDispatcher,
    pub select: Arc<dyn Fn() -> Result<SelectedSpeechAdapter, SpeechFailure> + Send + Sync>,
    pub clock: Arc<dyn SpeechClock>,
    pub events: Arc<dyn SpeechQueueEvents>,
}

#[cfg(test)]
mod tests;

impl SpeechQueueWorker {
    pub(crate) async fn run(&self) {
        loop {
            let dispatcher = self.dispatcher.clone();
            let dispatch_guard = dispatcher.lock_owned().await;
            let mut control_pending = false;
            let request = {
                let mut queue = match self.queue.lock() {
                    Ok(queue) => queue,
                    Err(error) => {
                        self.events.log(AppLogLevel::Error, error.to_string());
                        return;
                    }
                };
                if queue.controls_in_progress > 0 {
                    control_pending = true;
                    None
                } else if queue.paused {
                    queue.is_processing = false;
                    self.events.activity(
                        SpeechStatus::Paused,
                        Some("読み上げキューを一時停止しました。".to_string()),
                    );
                    self.events.snapshot(&queue, None);
                    return;
                } else if queue.pending.is_empty() {
                    queue.is_processing = false;
                    self.events.activity(
                        SpeechStatus::Idle,
                        Some("読み上げキューは空です。".to_string()),
                    );
                    self.events.snapshot(&queue, None);
                    return;
                } else {
                    let Some(request) = queue.reserve_next_request_after_dispatch_lock() else {
                        queue.is_processing = false;
                        self.events.snapshot(&queue, None);
                        return;
                    };
                    self.events.snapshot(&queue, None);
                    Some(request)
                }
            };

            let Some(request) = request else {
                drop(dispatch_guard);
                if control_pending {
                    if let Err(error) = self.wait_for_control().await {
                        self.events.log(AppLogLevel::Error, error);
                        return;
                    }
                    continue;
                }
                return;
            };

            self.events.activity(
                SpeechStatus::Speaking,
                Some("チャットを読み上げています。".to_string()),
            );
            let selected = (self.select)();
            let submitted = match &selected {
                Ok(adapter) => {
                    let session = adapter.session_after_dispatch_lock(dispatch_guard);
                    session.speak(request.clone()).await
                }
                Err(failure) => {
                    drop(dispatch_guard);
                    Err(failure.clone())
                }
            };
            let result = match (submitted, selected) {
                (Ok(_), Ok(adapter)) => Ok(adapter.wait_for_completion().await),
                (Err(failure), _) => Err(failure),
                (Ok(_), Err(_)) => unreachable!("submission requires a selected adapter"),
            };
            if let Err(error) = self.wait_for_control().await {
                self.events.log(AppLogLevel::Error, error);
                return;
            }
            match result {
                Ok(SpeechPlaybackCompletion::Completed) => {
                    let mut queue = match self.queue.lock() {
                        Ok(queue) => queue,
                        Err(error) => {
                            self.events.log(AppLogLevel::Error, error.to_string());
                            return;
                        }
                    };
                    queue.complete_request(&request.id);
                    self.events.snapshot(&queue, None);
                }
                Ok(SpeechPlaybackCompletion::Unconfirmed(failure)) => {
                    let message = format!("[{}] {}", request.id, failure.user_message);
                    let mut queue = match self.queue.lock() {
                        Ok(queue) => queue,
                        Err(error) => {
                            self.events.log(AppLogLevel::Error, error.to_string());
                            return;
                        }
                    };
                    if queue.fail_after_acceptance(&request.id, &failure) {
                        self.events
                            .health(failure.adapter_health(), Some(message.clone()));
                        self.events.log(
                            AppLogLevel::Error,
                            format!("[{}] {}", request.id, failure.log_message()),
                        );
                        self.events.snapshot(&queue, Some(message));
                    } else {
                        self.events.log(
                            AppLogLevel::Warning,
                            format!("[{}] 取消済みの読み上げは読み上げ先側の完了を確認できませんでした。", request.id),
                        );
                    }
                }
                Err(failure) => {
                    let error_message = &failure.user_message;
                    let transition;
                    {
                        let mut queue = match self.queue.lock() {
                            Ok(queue) => queue,
                            Err(error) => {
                                self.events.log(AppLogLevel::Error, error.to_string());
                                return;
                            }
                        };
                        transition = queue.fail_request_with_retry(&request.id, &failure);
                        if transition == SpeechQueueFailureTransition::Ignored {
                            self.events.log(
                                AppLogLevel::Warning,
                                format!(
                                    "[{}] 取消済みの読み上げ送信が失敗しました: {}",
                                    request.id,
                                    failure.log_message()
                                ),
                            );
                            continue;
                        }
                        let queue_message = match transition {
                            SpeechQueueFailureTransition::RetryScheduled => error_message.clone(),
                            SpeechQueueFailureTransition::RetryExhausted => {
                                let note = if failure.retryable {
                                    "自動再試行を終了しました。"
                                } else {
                                    "安全な自動再試行はできないため、再送していません。"
                                };
                                format!("{error_message} {note} エラー履歴へ移しました。状態を確認してからQueueの「再試行」を使ってください。")
                            }
                            SpeechQueueFailureTransition::Ignored => error_message.clone(),
                        };
                        let queue_message = format!("[{}] {queue_message}", request.id);
                        self.events
                            .health(failure.adapter_health(), Some(queue_message.clone()));
                        self.events.log(
                            AppLogLevel::Error,
                            format!("{queue_message} {}", failure.log_message()),
                        );
                        self.events.snapshot(&queue, Some(queue_message));
                    }
                    if transition == SpeechQueueFailureTransition::RetryScheduled {
                        self.clock.sleep(RETRY_DELAY).await;
                        let mut queue = match self.queue.lock() {
                            Ok(queue) => queue,
                            Err(error) => {
                                self.events.log(AppLogLevel::Error, error.to_string());
                                return;
                            }
                        };
                        queue.activate_scheduled_retry(&request.id);
                        self.events.snapshot(&queue, None);
                    }
                }
            }
        }
    }

    async fn wait_for_control(&self) -> Result<(), String> {
        loop {
            let controls_in_progress = {
                let controls_in_progress = self
                    .queue
                    .lock()
                    .map_err(|error| error.to_string())?
                    .controls_in_progress;
                controls_in_progress
            };
            if controls_in_progress == 0 {
                return Ok(());
            }
            self.clock.sleep(Duration::from_millis(10)).await;
        }
    }
}
