use super::{observer::Observer, AgentError, Executor, Worker};
use crate::jobs::JobUpdate;
use hmail_cleaner_core::cleanup::Cancellation;
use hmail_cleaner_protocol::{JobPhase, ObservedMailState, SummaryDto, ValidatedJob};
use hmail_cleaner_runner::RunObserver;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{mpsc, watch};
const SERVICE_CAPACITY: usize = 8;
const PERSIST_INTERVAL: Duration = Duration::from_secs(5);
impl<E: Executor> Worker<E> {
    pub(super) async fn run(&self, job: ValidatedJob, cancel: Arc<Cancellation>) {
        let id = job.command().job_id;
        if self.run_inner(&job, cancel).await.is_err() {
            self.0.healthy.store(false, Ordering::Release);
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
        let (progress, mut latest) = watch::channel(None);
        let (service, mut service_events) = mpsc::channel(SERVICE_CAPACITY);
        let observer = Observer {
            progress,
            service,
            failed: Arc::new(AtomicBool::new(false)),
            cancel: cancel.clone(),
        };
        let execution = self.0.executor.execute(job, &observer, &cancel);
        tokio::pin!(execution);
        let mut interval = tokio::time::interval(PERSIST_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut storage_failed = false;
        let result = loop {
            tokio::select! {
                result = &mut execution => break result,
                state = service_events.recv() => {
                    if let Some(state) = state {
                        let progress = *latest.borrow_and_update();
                        if self.observe(id, progress, Some(state)).await.is_err() { storage_failed = true; cancel.request(); }
                    }
                }
                _ = interval.tick(), if !storage_failed => {
                    if latest.has_changed().unwrap_or(false) {
                        let progress = *latest.borrow_and_update();
                        if self.observe(id, progress, None).await.is_err() { storage_failed = true; cancel.request(); }
                    }
                }
            }
        };
        while let Ok(state) = service_events.try_recv() {
            let progress = *latest.borrow_and_update();
            if self.observe(id, progress, Some(state)).await.is_err() {
                storage_failed = true;
            }
        }
        let fatal = result.is_fatal() || observer.failed() || storage_failed;
        self.finish(id, result.into_report(), fatal).await?;
        if storage_failed || observer.failed() {
            return Err(AgentError::Unavailable);
        }
        Ok(())
    }
}
