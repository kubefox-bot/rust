use super::{AgentError, Executor};
use crate::jobs::{AcceptOutcome, JobRepository};
use hmail_cleaner_core::cleanup::Cancellation;
use hmail_cleaner_protocol::{AgentId, JobId, JobSnapshot, ValidatedJob};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{broadcast, Mutex, Notify};
const EVENT_CAPACITY: usize = 16;
const HEALTH_READY_REVISION: u64 = 1;
const HEALTH_UNAVAILABLE_REVISION: u64 = 2;

pub(super) struct Active {
    pub id: JobId,
    pub cancel: Arc<Cancellation>,
}
pub(super) struct State<E> {
    pub agent_id: AgentId,
    pub version: String,
    pub repository: Arc<JobRepository>,
    pub executor: E,
    pub active: Mutex<Option<Active>>,
    pub events: broadcast::Sender<JobSnapshot>,
    pub healthy: AtomicBool,
    pub stopping: AtomicBool,
    pub idle: Notify,
}
pub struct Worker<E>(pub(super) Arc<State<E>>);
impl<E> Clone for Worker<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<E: Executor> Worker<E> {
    pub(crate) fn mark_unavailable(&self) {
        self.0.healthy.store(false, Ordering::Release);
    }
    pub fn new(agent_id: AgentId, repository: Arc<JobRepository>, executor: E) -> Self {
        Self::configured(
            agent_id,
            repository,
            executor,
            env!("CARGO_PKG_VERSION").into(),
        )
    }
    pub(crate) fn configured(
        agent_id: AgentId,
        repository: Arc<JobRepository>,
        executor: E,
        version: String,
    ) -> Self {
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        Self(Arc::new(State {
            agent_id,
            version,
            repository,
            executor,
            active: Mutex::new(None),
            events,
            healthy: AtomicBool::new(true),
            stopping: AtomicBool::new(false),
            idle: Notify::new(),
        }))
    }
    pub fn is_healthy(&self) -> bool {
        self.0.healthy.load(Ordering::Acquire) && !self.0.stopping.load(Ordering::Acquire)
    }
    pub fn subscribe(&self) -> broadcast::Receiver<JobSnapshot> {
        self.0.events.subscribe()
    }
    pub fn agent_id(&self) -> AgentId {
        self.0.agent_id
    }
    pub fn epoch(&self) -> uuid::Uuid {
        self.0.repository.epoch()
    }
    pub(crate) fn health(&self) -> hmail_cleaner_protocol::AgentHealth {
        let healthy = self.is_healthy();
        hmail_cleaner_protocol::AgentHealth {
            agent_id: self.agent_id(),
            version: self.0.version.clone(),
            protocol_min: hmail_cleaner_protocol::PROTOCOL_VERSION,
            protocol_max: hmail_cleaner_protocol::PROTOCOL_VERSION,
            stream_epoch: self.epoch(),
            revision: if healthy {
                HEALTH_READY_REVISION
            } else {
                HEALTH_UNAVAILABLE_REVISION
            }
            .into(),
            healthy,
        }
    }
    pub async fn lookup(&self, id: JobId) -> Result<Option<JobSnapshot>, AgentError> {
        Ok(self.0.repository.lookup(id).await?)
    }
    pub async fn submit(&self, job: ValidatedJob) -> Result<JobSnapshot, AgentError> {
        let worker = self.clone();
        // Dropping the HTTP future only drops its JoinHandle, not admission.
        tokio::spawn(async move { worker.admit(job).await })
            .await
            .map_err(|_| AgentError::Unavailable)?
    }
    async fn admit(&self, job: ValidatedJob) -> Result<JobSnapshot, AgentError> {
        if job.command().agent_id != self.0.agent_id {
            return Err(AgentError::Invalid);
        }
        let mut active = self.0.active.try_lock().map_err(|_| AgentError::Busy)?;
        if !self.is_healthy() {
            return Err(AgentError::Unavailable);
        }
        let id = job.command().job_id;
        if active.as_ref().is_some_and(|running| running.id != id) {
            return Err(AgentError::Busy);
        }
        let fingerprint = self.0.repository.fingerprint(&job);
        let outcome = self.0.repository.accept(&job, fingerprint).await?;
        let snapshot = match outcome {
            AcceptOutcome::Conflict => return Err(AgentError::Conflict),
            AcceptOutcome::Busy => return Err(AgentError::Busy),
            AcceptOutcome::Existing | AcceptOutcome::Cancelled | AcceptOutcome::New => self
                .lookup(id)
                .await
                .and_then(|snapshot| snapshot.ok_or(AgentError::Unavailable)),
        };
        if outcome == AcceptOutcome::New {
            let cancel = Arc::new(Cancellation::default());
            *active = Some(Active {
                id,
                cancel: cancel.clone(),
            });
            if let Ok(snapshot) = &snapshot {
                let _ = self.0.events.send(snapshot.clone());
            }
            let worker = self.clone();
            tokio::spawn(async move {
                worker.run(job, cancel).await;
            });
        }
        snapshot
    }
    pub async fn cancel(&self, id: JobId) -> Result<JobSnapshot, AgentError> {
        let worker = self.clone();
        tokio::spawn(async move { worker.cancel_owned(id).await })
            .await
            .map_err(|_| AgentError::Unavailable)?
    }
    async fn cancel_owned(&self, id: JobId) -> Result<JobSnapshot, AgentError> {
        let active = self.0.active.try_lock().map_err(|_| AgentError::Busy)?;
        let snapshot = self.0.repository.cancel(id).await?;
        if let Some(running) = active.as_ref().filter(|running| running.id == id) {
            running.cancel.request();
        }
        let _ = self.0.events.send(snapshot.clone());
        Ok(snapshot)
    }
    pub async fn shutdown(&self) {
        self.0.stopping.store(true, Ordering::Release);
        loop {
            let idle = self.0.idle.notified();
            {
                let active = self.0.active.lock().await;
                let Some(active) = active.as_ref() else {
                    return;
                };
                active.cancel.request();
            }
            idle.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmail_cleaner_runner::{Report, RunObserver, RunResult, Status};
    use std::time::Duration;
    use tokio::sync::Semaphore;
    struct Started(Arc<Semaphore>);
    impl Executor for Started {
        async fn execute(
            &self,
            _: &ValidatedJob,
            _: &impl RunObserver,
            _: &Cancellation,
        ) -> RunResult {
            self.0.add_permits(1);
            RunResult::Finished(Report {
                mode: None,
                status: Status::Complete,
                summary: Default::default(),
                stage: None,
                error: None,
                exit_code: 0,
            })
        }
    }
    #[tokio::test]
    async fn interrupted_http_admission_still_finishes_its_durable_receipt_and_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.sqlite");
        let repo = Arc::new(JobRepository::open(&path).await.unwrap());
        let started = Arc::new(Semaphore::new(0));
        let agent = AgentId::new();
        let worker = Worker::new(agent, repo.clone(), Started(started.clone()));
        let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", path.display()))
            .await
            .unwrap();
        let mut blocker = pool.begin().await.unwrap();
        sqlx::query("UPDATE coordination SET revision = revision WHERE id = 1")
            .execute(&mut *blocker)
            .await
            .unwrap();
        let mut command: hmail_cleaner_protocol::JobCommand =
            serde_json::from_str(include_str!("../../../protocol/tests/fixtures/job-v1.json"))
                .unwrap();
        command.agent_id = agent;
        let id = command.job_id;
        let request_worker = worker.clone();
        let request = tokio::spawn(async move {
            request_worker
                .submit(hmail_cleaner_protocol::validate_job(command).unwrap())
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if worker.0.active.try_lock().is_err() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        blocker.commit().await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), started.acquire())
            .await
            .expect("agent must retain ownership after the HTTP task ends")
            .unwrap()
            .forget();
        assert!(repo.lookup(id).await.unwrap().is_some());
        worker.shutdown().await;
        pool.close().await;
    }
}

#[cfg(test)]
mod health_tests {
    use super::*;
    #[tokio::test]
    async fn draining_health_changes_revision_without_claiming_readiness() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(
            JobRepository::open(&dir.path().join("jobs.sqlite"))
                .await
                .unwrap(),
        );
        let worker = Worker::new(AgentId::new(), repo, crate::execution::NativeExecutor);
        let ready = worker.health();
        assert!(ready.healthy);
        worker.shutdown().await;
        let draining = worker.health();
        assert!(!draining.healthy);
        assert!(draining.revision > ready.revision);
        assert_eq!(draining.stream_epoch, ready.stream_epoch);
        assert_eq!(worker.health().revision, draining.revision);
    }
}
