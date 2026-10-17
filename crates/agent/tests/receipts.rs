use hmail_cleaner_agent::jobs::{AcceptOutcome, JobRepository};
use hmail_cleaner_protocol::{validate_job, JobCommand, JobId, JobPhase};

fn fixture() -> JobCommand {
    serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap()
}

#[tokio::test]
async fn same_id_same_command_has_one_receipt_and_changed_profile_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    let repo = JobRepository::open(&dir.path().join("jobs.sqlite3"))
        .await
        .unwrap();
    let job = validate_job(fixture()).unwrap();
    assert_eq!(
        repo.accept(&job, repo.fingerprint(&job)).await.unwrap(),
        AcceptOutcome::New
    );
    assert_eq!(
        repo.accept(&job, repo.fingerprint(&job)).await.unwrap(),
        AcceptOutcome::Existing
    );
    let mut command = fixture();
    command.mail_profile.mysql.password =
        zeroize::Zeroizing::new("changed-profile-same-version".into());
    let changed = validate_job(command).unwrap();
    assert_eq!(
        repo.accept(&changed, repo.fingerprint(&changed))
            .await
            .unwrap(),
        AcceptOutcome::Conflict
    );
    let row = repo.lookup(job.command().job_id).await.unwrap().unwrap();
    assert_eq!(row.phase, JobPhase::Accepted);
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        assert!(!bytes
            .windows(b"fixture-mail-secret".len())
            .any(|part| part == b"fixture-mail-secret"));
        assert!(!bytes
            .windows(b"D:\\Mail".len())
            .any(|part| part == b"D:\\Mail"));
    }
}

#[tokio::test]
async fn cancel_unknown_blocks_late_accept() {
    let dir = tempfile::tempdir().unwrap();
    let repo = JobRepository::open(&dir.path().join("jobs.sqlite3"))
        .await
        .unwrap();
    let job = validate_job(fixture()).unwrap();
    assert_eq!(
        repo.cancel(job.command().job_id).await.unwrap().phase,
        JobPhase::Cancelled
    );
    assert_eq!(
        repo.accept(&job, repo.fingerprint(&job)).await.unwrap(),
        AcceptOutcome::Cancelled
    );
}

#[tokio::test]
async fn initial_service_observation_survives_stop_restore_and_agent_restart() {
    use hmail_cleaner_protocol::ObservedMailState;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite3");
    let repo = JobRepository::open(&path).await.unwrap();
    let job = validate_job(fixture()).unwrap();
    let id = job.command().job_id;
    repo.accept(&job, repo.fingerprint(&job)).await.unwrap();
    for state in [
        ObservedMailState::Running,
        ObservedMailState::Unknown,
        ObservedMailState::Stopped,
        ObservedMailState::Restoring,
    ] {
        repo.observe(id, None, Some(state)).await.unwrap();
    }
    repo.close().await;
    let restarted = JobRepository::open(&path).await.unwrap();
    restarted.recover_interrupted().await.unwrap();
    let snapshot = restarted.lookup(id).await.unwrap().unwrap();
    assert_eq!(
        snapshot.initial_mail_state,
        Some(ObservedMailState::Running)
    );
    assert_eq!(snapshot.mail_state, ObservedMailState::Unknown);
    assert_eq!(snapshot.phase, JobPhase::Interrupted);
}

#[tokio::test]
async fn restart_interrupts_without_resuming_and_preserves_tombstones() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite3");
    let job = validate_job(fixture()).unwrap();
    let unknown = JobId::new();
    let repo = JobRepository::open(&path).await.unwrap();
    let fingerprint = repo.fingerprint(&job);
    repo.accept(&job, fingerprint).await.unwrap();
    repo.cancel(unknown).await.unwrap();
    repo.close().await;
    let reopened = JobRepository::open(&path).await.unwrap();
    assert_eq!(reopened.fingerprint(&job), fingerprint);
    reopened.recover_interrupted().await.unwrap();
    assert_eq!(
        reopened
            .lookup(job.command().job_id)
            .await
            .unwrap()
            .unwrap()
            .phase,
        JobPhase::Interrupted
    );
    assert_eq!(
        reopened.lookup(unknown).await.unwrap().unwrap().phase,
        JobPhase::Cancelled
    );
}

#[tokio::test]
async fn concurrent_distinct_ids_admit_exactly_one_job() {
    let dir = tempfile::tempdir().unwrap();
    let repo = JobRepository::open(&dir.path().join("jobs.sqlite3"))
        .await
        .unwrap();
    let a = validate_job(fixture()).unwrap();
    let mut command = fixture();
    command.job_id = JobId::new();
    let b = validate_job(command).unwrap();
    let (one, two) = tokio::join!(
        repo.accept(&a, repo.fingerprint(&a)),
        repo.accept(&b, repo.fingerprint(&b))
    );
    let outcomes = [one.unwrap(), two.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| **result == AcceptOutcome::New)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| **result == AcceptOutcome::Busy)
            .count(),
        1
    );
}

#[tokio::test]
async fn corrupted_database_is_an_error_instead_of_an_empty_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite3");
    std::fs::write(&path, b"not a sqlite database").unwrap();
    assert!(JobRepository::open(&path).await.is_err());
}

#[tokio::test]
async fn deferred_commit_failure_never_admits_a_receipt() {
    use sqlx::Connection;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite3");
    let repo = JobRepository::open(&path).await.unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new().filename(&path);
    let mut connection = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    for statement in [
        "CREATE TABLE fixture_parent (id INTEGER PRIMARY KEY)",
        "CREATE TABLE fixture_child (parent INTEGER REFERENCES fixture_parent(id) DEFERRABLE INITIALLY DEFERRED)",
        "CREATE TRIGGER fixture_commit_failure AFTER INSERT ON jobs BEGIN INSERT INTO fixture_child(parent) VALUES (1); END",
    ] { sqlx::query(statement).execute(&mut connection).await.unwrap(); }
    let job = validate_job(fixture()).unwrap();
    assert!(repo.accept(&job, repo.fingerprint(&job)).await.is_err());
    assert!(repo.lookup(job.command().job_id).await.unwrap().is_none());
}

#[tokio::test]
async fn terminal_receipt_cannot_be_restarted_or_rewritten() {
    use hmail_cleaner_agent::jobs::JobUpdate;
    use hmail_cleaner_protocol::{ObservedMailState, SummaryDto};
    let dir = tempfile::tempdir().unwrap();
    let repo = JobRepository::open(&dir.path().join("jobs.sqlite3"))
        .await
        .unwrap();
    let job = validate_job(fixture()).unwrap();
    let id = job.command().job_id;
    repo.accept(&job, repo.fingerprint(&job)).await.unwrap();
    let update = |phase| JobUpdate {
        phase,
        stage: None,
        summary: SummaryDto::default(),
        mail_state: ObservedMailState::Unknown,
        error: None,
    };
    repo.transition(id, update(JobPhase::Running))
        .await
        .unwrap();
    let completed = repo
        .transition(id, update(JobPhase::Completed))
        .await
        .unwrap();
    assert_eq!(completed.revision.get(), 3);
    assert!(repo
        .transition(id, update(JobPhase::Running))
        .await
        .is_err());
    assert!(repo
        .transition(id, update(JobPhase::Completed))
        .await
        .is_err());
    assert_eq!(
        repo.lookup(id).await.unwrap().unwrap().phase,
        JobPhase::Completed
    );
}

#[tokio::test]
async fn lookup_distinguishes_absence_from_a_read_failure() {
    use sqlx::Connection;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite3");
    let repo = JobRepository::open(&path).await.unwrap();
    let id = JobId::new();
    assert!(repo.lookup(id).await.unwrap().is_none());
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
    )
    .await
    .unwrap();
    sqlx::query("DROP TABLE jobs")
        .execute(&mut connection)
        .await
        .unwrap();
    assert!(repo.lookup(id).await.is_err());
}

#[tokio::test]
async fn independent_connections_share_the_single_active_constraint() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite3");
    let one = JobRepository::open(&path).await.unwrap();
    let two = JobRepository::open(&path).await.unwrap();
    let a = validate_job(fixture()).unwrap();
    let mut command = fixture();
    command.job_id = JobId::new();
    let b = validate_job(command).unwrap();
    let (left, right) = tokio::join!(
        one.accept(&a, one.fingerprint(&a)),
        two.accept(&b, two.fingerprint(&b))
    );
    let outcomes = [left.unwrap(), right.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|value| **value == AcceptOutcome::New)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|value| **value == AcceptOutcome::Busy)
            .count(),
        1
    );
}

#[tokio::test]
async fn maintenance_gate_is_durable_and_cannot_freeze_an_active_job() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite");
    let repo = JobRepository::open(&path).await.unwrap();
    assert!(repo.begin_maintenance().await.unwrap());
    let job = validate_job(fixture()).unwrap();
    assert_eq!(
        repo.accept(&job, repo.fingerprint(&job)).await.unwrap(),
        AcceptOutcome::Busy
    );
    repo.close().await;
    let repo = JobRepository::open(&path).await.unwrap();
    assert!(repo.in_maintenance().await.unwrap());
    repo.end_maintenance().await.unwrap();
    assert_eq!(
        repo.accept(&job, repo.fingerprint(&job)).await.unwrap(),
        AcceptOutcome::New
    );
    assert!(!repo.begin_maintenance().await.unwrap());
    assert!(!repo.in_maintenance().await.unwrap());
}
