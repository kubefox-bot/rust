use hmail_cleaner_agent::execution::Executor;
use hmail_cleaner_core::cleanup::{Cancellation, Summary};
use hmail_cleaner_protocol::{validate_job, AgentId, JobCommand, ValidatedJob};
use hmail_cleaner_runner::{MailObservation, Report, RunObserver, RunResult, Status};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::Semaphore;

pub fn job(agent: AgentId) -> ValidatedJob {
    let mut command: JobCommand =
        serde_json::from_str(include_str!("../../../protocol/tests/fixtures/job-v1.json")).unwrap();
    command.agent_id = agent;
    validate_job(command).unwrap()
}

#[derive(Clone)]
pub struct Controlled {
    pub calls: Arc<AtomicUsize>,
    pub started: Arc<Semaphore>,
    pub finish: Arc<Semaphore>,
    pub restoring: Arc<Semaphore>,
    pub restore: bool,
}
impl Controlled {
    pub fn new(restore: bool) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            started: Arc::new(Semaphore::new(0)),
            finish: Arc::new(Semaphore::new(0)),
            restoring: Arc::new(Semaphore::new(0)),
            restore,
        }
    }
}
impl Executor for Controlled {
    async fn execute(
        &self,
        _: &ValidatedJob,
        observer: &impl RunObserver,
        cancel: &Cancellation,
    ) -> RunResult {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.started.add_permits(1);
        if self.restore {
            while !cancel.is_requested() {
                tokio::task::yield_now().await;
            }
            observer.service_state(MailObservation::Restoring);
            self.restoring.add_permits(1);
        }
        self.finish.acquire().await.unwrap().forget();
        if self.restore {
            observer.service_state(MailObservation::State(
                hmail_cleaner_core::maintenance::mail_service::MailState::Running,
            ));
        }
        RunResult::Finished(Report {
            mode: None,
            status: if cancel.is_requested() {
                Status::Cancelled
            } else {
                Status::Complete
            },
            summary: Summary::default(),
            stage: None,
            error: None,
            exit_code: 0,
        })
    }
}
