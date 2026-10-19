use hmail_cleaner_agent::jobs::{JobRepository, JobStoreError, JobUpdate};
use hmail_cleaner_protocol::{
    validate_job, JobCommand, JobId, JobPhase, ObservedMailState, PreviewFile, PreviewFilesQuery,
    SummaryDto,
};

async fn accept_preview(repo: &JobRepository) -> JobId {
    let mut command: JobCommand =
        serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap();
    command.job_id = JobId::new();
    let job = validate_job(command).unwrap();
    let id = job.command().job_id;
    repo.accept(&job, repo.fingerprint(&job)).await.unwrap();
    repo.append_preview_files(
        id,
        &[PreviewFile {
            id: "file".into(),
            path: "D:/Mail/file.eml".into(),
            size_bytes: 7.into(),
            created_at: chrono::DateTime::UNIX_EPOCH,
            orphan: true,
        }],
    )
    .await
    .unwrap();
    id
}

async fn transition(repo: &JobRepository, id: JobId, phase: JobPhase) {
    repo.transition(
        id,
        JobUpdate {
            phase,
            stage: None,
            summary: SummaryDto::default(),
            mail_state: ObservedMailState::Unknown,
            error: None,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn old_catalogs_expire_without_losing_receipts_or_the_active_preview() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite");
    let repo = JobRepository::open(&path).await.unwrap();
    let mut finished = Vec::new();
    for phase in [
        JobPhase::Completed,
        JobPhase::Failed,
        JobPhase::Partial,
        JobPhase::Cancelled,
        JobPhase::Completed,
    ] {
        let id = accept_preview(&repo).await;
        transition(&repo, id, JobPhase::Running).await;
        transition(&repo, id, phase).await;
        finished.push(id);
    }
    let active = accept_preview(&repo).await;
    for id in &finished[..2] {
        assert!(matches!(
            repo.preview_files(*id, &PreviewFilesQuery::default()).await,
            Err(JobStoreError::PreviewUnavailable)
        ));
        assert!(!repo.lookup(*id).await.unwrap().unwrap().phase.is_active());
    }
    for id in finished[2..].iter().chain(std::iter::once(&active)) {
        assert_eq!(
            repo.preview_files(*id, &PreviewFilesQuery::default())
                .await
                .unwrap()
                .total
                .get(),
            1
        );
    }
    repo.close().await;
    let repo = JobRepository::open(&path).await.unwrap();
    repo.recover_interrupted().await.unwrap();
    assert_eq!(
        repo.lookup(active).await.unwrap().unwrap().phase,
        JobPhase::Interrupted
    );
    assert!(matches!(
        repo.preview_files(finished[2], &PreviewFilesQuery::default())
            .await,
        Err(JobStoreError::PreviewUnavailable)
    ));
    assert_eq!(
        repo.preview_files(active, &PreviewFilesQuery::default())
            .await
            .unwrap()
            .total
            .get(),
        1
    );
    let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    let catalogs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM preview_catalogs")
        .fetch_one(&pool)
        .await
        .unwrap();
    let files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM preview_files")
        .fetch_one(&pool)
        .await
        .unwrap();
    let receipts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((catalogs, files, receipts), (3, 3, 6));
    pool.close().await;
}

#[tokio::test]
async fn cancelling_an_accepted_preview_also_enforces_retention() {
    let dir = tempfile::tempdir().unwrap();
    let repo = JobRepository::open(&dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    let first = accept_preview(&repo).await;
    transition(&repo, first, JobPhase::Running).await;
    transition(&repo, first, JobPhase::Completed).await;
    for _ in 0..2 {
        let id = accept_preview(&repo).await;
        transition(&repo, id, JobPhase::Running).await;
        transition(&repo, id, JobPhase::Completed).await;
    }
    let cancelled = accept_preview(&repo).await;
    assert_eq!(
        repo.cancel(cancelled).await.unwrap().phase,
        JobPhase::Cancelled
    );
    assert!(matches!(
        repo.preview_files(first, &PreviewFilesQuery::default())
            .await,
        Err(JobStoreError::PreviewUnavailable)
    ));
    assert_eq!(
        repo.preview_files(cancelled, &PreviewFilesQuery::default())
            .await
            .unwrap()
            .total
            .get(),
        1
    );
    assert_eq!(
        repo.lookup(first).await.unwrap().unwrap().phase,
        JobPhase::Completed
    );
}

#[tokio::test]
async fn failed_catalog_expiry_rolls_back_the_terminal_receipt_and_can_be_retried() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite");
    let repo = JobRepository::open(&path).await.unwrap();
    let first = accept_preview(&repo).await;
    transition(&repo, first, JobPhase::Running).await;
    transition(&repo, first, JobPhase::Completed).await;
    for _ in 0..2 {
        let id = accept_preview(&repo).await;
        transition(&repo, id, JobPhase::Running).await;
        transition(&repo, id, JobPhase::Completed).await;
    }
    let current = accept_preview(&repo).await;
    transition(&repo, current, JobPhase::Running).await;
    let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER deny_expiry BEFORE DELETE ON preview_files BEGIN SELECT RAISE(ABORT, 'fixture expiry failure'); END")
        .execute(&pool).await.unwrap();
    let terminal = JobUpdate {
        phase: JobPhase::Completed,
        stage: None,
        summary: SummaryDto::default(),
        mail_state: ObservedMailState::Unknown,
        error: None,
    };
    assert!(repo.transition(current, terminal).await.is_err());
    assert_eq!(
        repo.lookup(current).await.unwrap().unwrap().phase,
        JobPhase::Running
    );
    assert_eq!(
        repo.preview_files(first, &PreviewFilesQuery::default())
            .await
            .unwrap()
            .total
            .get(),
        1
    );
    sqlx::query("DROP TRIGGER deny_expiry")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    transition(&repo, current, JobPhase::Completed).await;
    assert!(matches!(
        repo.preview_files(first, &PreviewFilesQuery::default())
            .await,
        Err(JobStoreError::PreviewUnavailable)
    ));
    assert_eq!(
        repo.lookup(current).await.unwrap().unwrap().phase,
        JobPhase::Completed
    );
}
