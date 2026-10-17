use super::mapping;
use hmail_cleaner_core::cleanup::{Cancellation, Progress};
use hmail_cleaner_protocol::{JobStage, SummaryDto};
use hmail_cleaner_runner::{MailObservation, RunObserver};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{mpsc, watch};

#[derive(Clone, Copy)]
pub(super) struct ProgressValue {
    pub stage: JobStage,
    pub summary: SummaryDto,
}
pub(super) struct Observer {
    pub progress: watch::Sender<Option<ProgressValue>>,
    pub service: mpsc::Sender<MailObservation>,
    pub failed: Arc<AtomicBool>,
    pub cancel: Arc<Cancellation>,
}
impl RunObserver for Observer {
    fn progress(&self, progress: Progress) {
        // Raw failure messages and paths never cross the public journal boundary.
        self.progress.send_replace(Some(ProgressValue {
            stage: mapping::stage(progress.stage),
            summary: mapping::summary(&progress.summary),
        }));
    }
    fn service_state(&self, state: MailObservation) {
        if self.service.try_send(state).is_err() {
            self.failed.store(true, Ordering::Release);
            self.cancel.request();
        }
    }
    fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}
