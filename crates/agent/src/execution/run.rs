use super::{
    diagnostics::{self, Event},
    observer::Observer,
    AgentError, Executor, Worker,
};
use crate::jobs::JobUpdate;
use hmail_cleaner_core::cleanup::Cancellation;
use hmail_cleaner_protocol::{JobPhase, ObservedMailState, SummaryDto, ValidatedJob};
use hmail_cleaner_runner::RunObserver;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::{mpsc, watch};
const SERVICE_CAPACITY: usize = 8;
impl<E: Executor> Worker<E> {
    pub(super) async fn run(&self, job: ValidatedJob, cancel: Arc<Cancellation>) {
        let id = job.command().job_id;
        if self.run_inner(&job, cancel).await.is_err() {
            diagnostics::record(self.0.journal.as_ref(), Event::ReceiptFailed { job_id: id });
            self.mark_unavailable();
        }
        let mut active = self.0.active.lock().await;
        if active.as_ref().is_some_and(|active| active.id == id) {
            *active = None;
        }
        self.0.idle.notify_waiters();
    }
    async fn run_inner(
        &self,
        job: &ValidatedJob,
        cancel: Arc<Cancellation>,
    ) -> Result<(), AgentError> {
        let id = job.command().job_id;
        // transition and cancel serialize in SQLite; a Cancelled receipt cannot be claimed.
        let claimed = self
            .0
            .repository
            .transition(
                id,
                JobUpdate {
                    phase: JobPhase::Running,
                    stage: None,
                    summary: SummaryDto::default(),
                    mail_state: ObservedMailState::Unknown,
                    error: None,
                },
            )
            .await;
        let claimed = match claimed {
            Ok(claimed) => claimed,
            Err(crate::jobs::JobStoreError::InvalidTransition)
                if self
                    .lookup(id)
                    .await?
                    .is_some_and(|snapshot| snapshot.phase == JobPhase::Cancelled) =>
            {
                return Ok(())
            }
            Err(error) => return Err(error.into()),
        };
        let _ = self.0.events.send(claimed);
        let (progress, latest) = watch::channel(None);
        let (service, service_events) = mpsc::channel(SERVICE_CAPACITY);
        diagnostics::record(self.0.journal.as_ref(), Event::JobStarted { job_id: id });
        let observer = Observer {
            repository: self.0.repository.clone(),
            events: self.0.events.clone(),
            job_id: id,
            journal: self.0.journal.clone(),
            last_stage: std::sync::Mutex::new(None),
            progress,
            service,
            failed: Arc::new(AtomicBool::new(false)),
            cancel: cancel.clone(),
        };
        let execution = async {
            let result = self.0.executor.execute(job, &observer, &cancel).await;
            let output_failed = observer.failed();
            // Closing the sender lets persistence drain every service observation before finish.
            drop(observer);
            (result, output_failed)
        };
        // Both futures keep advancing while either waits for the single SQLite writer.
        let ((result, output_failed), storage_failed) = tokio::join!(
            execution,
            self.persist_progress(id, &cancel, latest, service_events),
        );
        let fatal = result.is_fatal() || output_failed || storage_failed;
        self.finish(id, result.into_report(), fatal).await?;
        if storage_failed || output_failed {
            return Err(AgentError::Unavailable);
        }
        Ok(())
    }
}
