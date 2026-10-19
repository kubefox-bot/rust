mod fixtures;
use fixtures::{job, Controlled};
use hmail_cleaner_agent::{execution::Worker, jobs::JobRepository};
use hmail_cleaner_protocol::{AgentId, JobPhase};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
const TIMEOUT: Duration = Duration::from_secs(5);

async fn terminal(worker: &Worker<Controlled>, id: hmail_cleaner_protocol::JobId) -> JobPhase {
    tokio::time::timeout(TIMEOUT, async {
        loop {
            let snapshot = worker.lookup(id).await.unwrap().unwrap();
            if !snapshot.phase.is_active() {
                return snapshot.phase;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn receipt_precedes_execution_and_repeated_id_runs_once() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let agent = AgentId::new();
    let executor = Controlled::new(false);
    let worker = Worker::new(agent, repo.clone(), executor.clone());
    let command = job(agent);
    let id = command.command().job_id;
    worker.submit(command).await.unwrap();
    executor.started.acquire().await.unwrap().forget();
    assert!(
        repo.lookup(id).await.unwrap().is_some(),
        "receipt must already be durable"
    );
    worker.submit(job(agent)).await.unwrap();
    executor.finish.add_permits(1);
    assert_eq!(terminal(&worker, id).await, JobPhase::Completed);
    worker.submit(job(agent)).await.unwrap();
    assert_eq!(executor.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn wrong_agent_never_starts_runner_or_creates_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let executor = Controlled::new(false);
    let worker = Worker::new(AgentId::new(), repo.clone(), executor.clone());
    let command = job(AgentId::new());
    let id = command.command().job_id;
    assert!(worker.submit(command).await.is_err());
    assert!(repo.lookup(id).await.unwrap().is_none());
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn dropping_event_connection_does_not_cancel_work() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let agent = AgentId::new();
    let executor = Controlled::new(false);
    let worker = Worker::new(agent, repo, executor.clone());
    let stream = worker.subscribe();
    let command = job(agent);
    let id = command.command().job_id;
    worker.submit(command).await.unwrap();
    drop(stream);
    executor.started.acquire().await.unwrap().forget();
    executor.finish.add_permits(1);
    assert_eq!(terminal(&worker, id).await, JobPhase::Completed);
}

#[tokio::test]
async fn cancellation_is_nonterminal_until_restore_finishes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let agent = AgentId::new();
    let executor = Controlled::new(true);
    let worker = Worker::new(agent, repo, executor.clone());
    let command = job(agent);
    let id = command.command().job_id;
    worker.submit(command).await.unwrap();
    executor.started.acquire().await.unwrap().forget();
    worker.cancel(id).await.unwrap();
    executor.restoring.acquire().await.unwrap().forget();
    tokio::time::timeout(TIMEOUT, async {
        while worker.lookup(id).await.unwrap().unwrap().phase != JobPhase::Restoring {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(worker.lookup(id).await.unwrap().unwrap().phase.is_active());
    executor.finish.add_permits(1);
    assert_eq!(terminal(&worker, id).await, JobPhase::Cancelled);
}

#[tokio::test]
async fn terminal_commit_failure_never_publishes_completed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite");
    let repo = Arc::new(JobRepository::open(&path).await.unwrap());
    let agent = AgentId::new();
    let executor = Controlled::new(false);
    let worker = Worker::new(agent, repo, executor.clone());
    let mut events = worker.subscribe();
    let command = job(agent);
    let id = command.command().job_id;
    worker.submit(command).await.unwrap();
    executor.started.acquire().await.unwrap().forget();
    let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    for statement in [
        "CREATE TABLE fixture_parent (id INTEGER PRIMARY KEY)",
        "CREATE TABLE fixture_child (parent INTEGER REFERENCES fixture_parent(id) DEFERRABLE INITIALLY DEFERRED)",
        "CREATE TRIGGER fail_terminal AFTER UPDATE ON jobs WHEN NEW.phase = 'completed' BEGIN INSERT INTO fixture_child(parent) VALUES (1); END",
    ] { sqlx::query(statement).execute(&pool).await.unwrap(); }
    executor.finish.add_permits(1);
    tokio::time::timeout(TIMEOUT, async {
        while worker.is_healthy() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    while let Ok(snapshot) = events.try_recv() {
        assert_ne!(snapshot.phase, JobPhase::Completed);
    }
    assert_ne!(
        worker.lookup(id).await.unwrap().unwrap().phase,
        JobPhase::Completed
    );
    assert!(worker.submit(job(agent)).await.is_err());
    pool.close().await;
}

#[path = "fixtures/database.rs"]
mod database;

struct CountNative(Arc<std::sync::atomic::AtomicUsize>);
impl hmail_cleaner_agent::execution::Executor for CountNative {
    async fn execute(
        &self,
        job: &hmail_cleaner_protocol::ValidatedJob,
        observer: &impl hmail_cleaner_runner::RunObserver,
        cancel: &hmail_cleaner_core::cleanup::Cancellation,
    ) -> hmail_cleaner_runner::RunResult {
        self.0.fetch_add(1, Ordering::SeqCst);
        hmail_cleaner_agent::execution::execute(job, observer, cancel).await
    }
}
#[tokio::test]
#[ignore = "requires disposable MySQL via mise run test:agent-db"]
async fn database_preview_preserves_real_files_and_rows_and_duplicate_is_not_executed() {
    let mut database = database::Database::create().await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Data");
    std::fs::create_dir(&root).unwrap();
    let filename = "{00000000-0000-4000-8000-000000000001}.eml";
    let message = root.join(filename);
    std::fs::write(&message, b"fixture message").unwrap();
    let ini = dir.path().join("hMailServer.ini");
    std::fs::write(
        &ini,
        format!("[Directories]\nDataFolder={}\n", root.display()),
    )
    .unwrap();
    sqlx::query("INSERT INTO hm_messages (messageid,messageaccountid,messagefolderid,messagefilename,messagetype,messagefrom,messagesize,messagecurnooftries,messagenexttrytime,messageflags,messagecreatetime,messagelocked,messageuid) VALUES (1,0,0,?,0,'fixture@example.invalid',15,0,'2026-01-01',0,'2026-01-01',0,1)").bind(filename).execute(&mut database.admin).await.unwrap();
    let agent = AgentId::new();
    let mut command = job(agent).into_command();
    command.mail_profile.mysql = database.parts.clone();
    command.mail_profile.data_dir = root.to_str().unwrap().into();
    command.mail_profile.hmail_ini = ini.to_str().unwrap().into();
    command.resolved_cutoff = hmail_cleaner_agent::cutoff::resolve(
        &chrono::Utc,
        chrono::Utc::now(),
        hmail_cleaner_protocol::RetentionRequest::Before {
            local: chrono::NaiveDate::from_ymd_opt(2099, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        },
    )
    .unwrap();
    let id = command.job_id;
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let worker = Worker::new(agent, repo, CountNative(calls.clone()));
    worker
        .submit(hmail_cleaner_protocol::validate_job(command.clone()).unwrap())
        .await
        .unwrap();
    let final_snapshot = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let snapshot = worker.lookup(id).await.unwrap().unwrap();
            if !snapshot.phase.is_active() {
                break snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        final_snapshot.phase,
        JobPhase::Completed,
        "{final_snapshot:?}"
    );
    assert_eq!(final_snapshot.summary.selected.get(), 1);
    assert_eq!(final_snapshot.summary.deleted_messages.get(), 0);
    assert!(final_snapshot.summary.remaining_old_files.is_none());
    assert!(message.exists());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM hm_messages")
            .fetch_one(&mut database.admin)
            .await
            .unwrap(),
        1
    );
    worker
        .submit(hmail_cleaner_protocol::validate_job(command).unwrap())
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    worker.shutdown().await;
    database.finish().await;
}

struct OverflowObserver;
impl hmail_cleaner_agent::execution::Executor for OverflowObserver {
    async fn execute(
        &self,
        _: &hmail_cleaner_protocol::ValidatedJob,
        observer: &impl hmail_cleaner_runner::RunObserver,
        cancel: &hmail_cleaner_core::cleanup::Cancellation,
    ) -> hmail_cleaner_runner::RunResult {
        for _ in 0..32 {
            observer.service_state(hmail_cleaner_runner::MailObservation::Restoring);
        }
        assert!(
            cancel.is_requested(),
            "mandatory journal overflow must request safe cancellation"
        );
        hmail_cleaner_runner::RunResult::Finished(hmail_cleaner_runner::Report {
            mode: None,
            status: hmail_cleaner_runner::Status::Complete,
            summary: Default::default(),
            stage: None,
            error: None,
            exit_code: 0,
        })
    }
}
#[tokio::test]
async fn mandatory_progress_sink_failure_cannot_report_success() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let agent = AgentId::new();
    let worker = Worker::new(agent, repo, OverflowObserver);
    let command = job(agent);
    let id = command.command().job_id;
    worker.submit(command).await.unwrap();
    tokio::time::timeout(TIMEOUT, async {
        loop {
            let snapshot = worker.lookup(id).await.unwrap().unwrap();
            if !snapshot.phase.is_active() {
                assert_eq!(snapshot.phase, JobPhase::Failed);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

struct UnconfirmedRestore;
impl hmail_cleaner_agent::execution::Executor for UnconfirmedRestore {
    async fn execute(
        &self,
        _: &hmail_cleaner_protocol::ValidatedJob,
        observer: &impl hmail_cleaner_runner::RunObserver,
        _: &hmail_cleaner_core::cleanup::Cancellation,
    ) -> hmail_cleaner_runner::RunResult {
        observer.service_state(hmail_cleaner_runner::MailObservation::Restoring);
        hmail_cleaner_runner::RunResult::Finished(hmail_cleaner_runner::Report {
            mode: None,
            status: hmail_cleaner_runner::Status::Complete,
            summary: Default::default(),
            stage: None,
            error: None,
            exit_code: 0,
        })
    }
}
#[tokio::test]
async fn unconfirmed_restoration_is_never_reported_as_completed() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let agent = AgentId::new();
    let worker = Worker::new(agent, repo, UnconfirmedRestore);
    let command = job(agent);
    let id = command.command().job_id;
    worker.submit(command).await.unwrap();
    tokio::time::timeout(TIMEOUT, async {
        loop {
            let snapshot = worker.lookup(id).await.unwrap().unwrap();
            if !snapshot.phase.is_active() {
                assert_eq!(snapshot.phase, JobPhase::Failed);
                assert_eq!(
                    snapshot.error.unwrap().code(),
                    hmail_cleaner_protocol::ErrorCode::RestoreFailed
                );
                assert_eq!(
                    snapshot.mail_state,
                    hmail_cleaner_protocol::ObservedMailState::Unknown
                );
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
