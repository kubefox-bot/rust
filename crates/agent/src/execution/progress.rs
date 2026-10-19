use super::{observer::ProgressValue, Executor, Worker};
use hmail_cleaner_core::cleanup::Cancellation;
use hmail_cleaner_protocol::JobId;
use hmail_cleaner_runner::MailObservation;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

const PERSIST_INTERVAL: Duration = Duration::from_secs(5);

impl<E: Executor> Worker<E> {
    pub(super) async fn persist_progress(
        &self,
        id: JobId,
        cancel: &Cancellation,
        mut latest: watch::Receiver<Option<ProgressValue>>,
        mut service_events: mpsc::Receiver<MailObservation>,
    ) -> bool {
        let mut interval = tokio::time::interval(PERSIST_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut storage_failed = false;
        loop {
            let service = tokio::select! {
                state = service_events.recv() => match state {
                    Some(state) => Some(state),
                    None => return storage_failed,
                },
                _ = interval.tick(), if !storage_failed => {
                    if !latest.has_changed().unwrap_or(false) { continue; }
                    None
                }
            };
            let progress = *latest.borrow_and_update();
            if self.observe(id, progress, service).await.is_err() {
                storage_failed = true;
                cancel.request();
            }
        }
    }
}
