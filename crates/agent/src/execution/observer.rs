use super::{
    diagnostics::{self, Event},
    mapping,
};
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
    pub job_id: hmail_cleaner_protocol::JobId,
    pub journal: Option<hmail_cleaner_adapters::journal::RuntimeJournal>,
    pub last_stage: std::sync::Mutex<Option<hmail_cleaner_core::cleanup::Stage>>,
    pub progress: watch::Sender<Option<ProgressValue>>,
    pub service: mpsc::Sender<MailObservation>,
    pub failed: Arc<AtomicBool>,
    pub cancel: Arc<Cancellation>,
}
impl RunObserver for Observer {
    fn progress(&self, progress: Progress) {
        if let Ok(mut stage) = self.last_stage.lock() {
            if *stage != Some(progress.stage) {
                diagnostics::record(
                    self.journal.as_ref(),
                    Event::StageChanged {
                        job_id: self.job_id,
                        stage: progress.stage,
                    },
                );
                *stage = Some(progress.stage);
            }
        }
        if let Some(failure) = &progress.failure {
            let (failure, message_id) = diagnostics::failure(failure);
            diagnostics::record(
                self.journal.as_ref(),
                Event::CleanupFailure {
                    job_id: self.job_id,
                    stage: progress.stage,
                    failure,
                    message_id,
                    failed: progress.summary.failed,
                },
            );
        }
        // Only counters coalesce; safe failures are already written independently.
        self.progress.send_replace(Some(ProgressValue {
            stage: mapping::stage(progress.stage),
            summary: mapping::summary(&progress.summary),
        }));
    }
    fn service_state(&self, state: MailObservation) {
        let observed = match state {
            MailObservation::Restoring => hmail_cleaner_protocol::ObservedMailState::Restoring,
            MailObservation::State(state) => state.into(),
        };
        diagnostics::record(
            self.journal.as_ref(),
            Event::MailState {
                job_id: self.job_id,
                state: observed,
            },
        );
        if self.service.try_send(state).is_err() {
            self.failed.store(true, Ordering::Release);
            self.cancel.request();
        }
    }
    fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}
