use hmail_cleaner_agent::jobs::JobRepository;
use hmail_cleaner_protocol::{
    validate_job, JobCommand, JobId, PreviewFile, PreviewFileSort, PreviewFilesQuery, SortDirection,
};

#[tokio::test]
async fn catalog_is_persistent_bounded_sorted_and_literal_searchable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite");
    let repo = JobRepository::open(&path).await.unwrap();
    let command: JobCommand =
        serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap();
    let job = validate_job(command).unwrap();
    let id = job.command().job_id;
    repo.accept(&job, repo.fingerprint(&job)).await.unwrap();
    let empty = repo
        .preview_files(id, &PreviewFilesQuery::default())
        .await
        .unwrap();
    assert_eq!(empty.total.get(), 0);
    assert!(empty.rows.is_empty());
    assert!(!empty.complete);
    let rows = [
        ("b", "D:/Mail/B%.eml", 3),
        ("a", "D:/Mail/a_.eml", 9),
        ("c", "D:/Mail/c.eml", 3),
    ]
    .into_iter()
    .map(|(id, path, size)| PreviewFile {
        id: id.into(),
        path: path.into(),
        size_bytes: size.into(),
        created_at: chrono::DateTime::from_timestamp(i64::from(id.as_bytes()[0]), 0).unwrap(),
        orphan: true,
    })
    .collect::<Vec<_>>();
    repo.append_preview_files(id, &rows).await.unwrap();
    assert_eq!(
        repo.accept(&job, repo.fingerprint(&job)).await.unwrap(),
        hmail_cleaner_agent::jobs::AcceptOutcome::Existing
    );
    let query = PreviewFilesQuery {
        sort: PreviewFileSort::Size,
        direction: SortDirection::Desc,
        limit: 1,
        ..Default::default()
    };
    let page = repo.preview_files(id, &query).await.unwrap();
    assert_eq!(page.total.get(), 3);
    assert_eq!(page.rows[0].id, "a");
    assert!(!page.complete);
    let tied = repo
        .preview_files(
            id,
            &PreviewFilesQuery {
                sort: PreviewFileSort::Size,
                limit: 2,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        tied.rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["b", "c"]
    );
    let by_created = repo
        .preview_files(
            id,
            &PreviewFilesQuery {
                sort: PreviewFileSort::CreatedAt,
                direction: SortDirection::Desc,
                limit: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(by_created.rows[0].id, "c");
    let page = repo
        .preview_files(
            id,
            &PreviewFilesQuery {
                search: "B%".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total.get(), 1);
    assert_eq!(page.rows[0].id, "b");
    let page = repo
        .preview_files(
            id,
            &PreviewFilesQuery {
                offset: 100,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(page.rows.is_empty());
    assert_eq!(page.total.get(), 3);
    assert!(repo
        .preview_files(JobId::new(), &PreviewFilesQuery::default())
        .await
        .is_err());
    repo.close().await;
    let repo = JobRepository::open(&path).await.unwrap();
    assert_eq!(
        repo.preview_files(id, &PreviewFilesQuery::default())
            .await
            .unwrap()
            .rows
            .len(),
        3
    );
}

#[tokio::test]
async fn endpoint_rejects_invalid_queries_missing_jobs_and_clean_jobs() {
    use hmail_cleaner_agent::{
        execution::{NativeExecutor, Worker},
        http,
    };
    use hmail_cleaner_protocol::{AgentId, Operation};
    use std::sync::Arc;
    use tower::ServiceExt;
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let mut command: JobCommand =
        serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap();
    command.operation = Operation::Clean;
    let job = validate_job(command).unwrap();
    let id = job.command().job_id;
    repo.accept(&job, repo.fingerprint(&job)).await.unwrap();
    let router = http::router(Worker::new(AgentId::new(), repo, NativeExecutor));
    for (uri, status) in [
        (format!("/agent/v1/jobs/{id}/files?limit=0"), 400),
        (
            format!("/agent/v1/jobs/{id}/files?offset=18446744073709551615"),
            400,
        ),
        (format!("/agent/v1/jobs/{id}/files?sort=unknown"), 400),
        (
            format!("/agent/v1/jobs/{id}/files?created_from=invalid"),
            400,
        ),
        (format!("/agent/v1/jobs/{id}/files?orphan=unknown"), 400),
        (format!("/agent/v1/jobs/{id}/files"), 400),
        (format!("/agent/v1/jobs/{}/files", JobId::new()), 404),
    ] {
        let response = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(uri)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status);
    }
}

#[tokio::test]
async fn awaited_observation_persists_and_broadcasts_before_executor_continues() {
    use hmail_cleaner_agent::execution::{Executor, Worker};
    use hmail_cleaner_core::{
        cleanup::Cancellation,
        maintenance::{recursive::SelectedFile, FileObservation},
    };
    use hmail_cleaner_protocol::{AgentId, ValidatedJob};
    use hmail_cleaner_runner::{Report, RunObserver, RunResult, Status};
    use std::{
        sync::Arc,
        time::{Duration, SystemTime},
    };
    use tokio::sync::Semaphore;
    struct Selected(Arc<Semaphore>);
    impl Executor for Selected {
        async fn execute(
            &self,
            _: &ValidatedJob,
            observer: &impl RunObserver,
            _: &Cancellation,
        ) -> RunResult {
            observer
                .selected_files(&[SelectedFile {
                    file: FileObservation {
                        path: "D:/Mail/a.eml".into(),
                        guid: uuid::Uuid::nil(),
                        size_bytes: 7,
                        modified_at: SystemTime::UNIX_EPOCH,
                        created_at: SystemTime::UNIX_EPOCH,
                    },
                    orphan: false,
                }])
                .await
                .unwrap();
            self.0.acquire().await.unwrap().forget();
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
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let gate = Arc::new(Semaphore::new(0));
    let agent = AgentId::new();
    let worker = Worker::new(agent, repo.clone(), Selected(gate.clone()));
    let mut command: JobCommand =
        serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap();
    command.agent_id = agent;
    let id = command.job_id;
    let mut events = worker.subscribe();
    worker.submit(validate_job(command).unwrap()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = events.recv().await.unwrap();
            let page = worker
                .preview_files(id, &PreviewFilesQuery::default())
                .await
                .unwrap();
            if !page.rows.is_empty() {
                assert!(!page.complete);
                assert!(snapshot.phase.is_active());
                assert_eq!(page.rows[0].size_bytes.get(), 7);
                assert!(!page.rows[0].orphan);
                if snapshot.revision == page.revision {
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
    gate.add_permits(1);
    loop {
        if !events.recv().await.unwrap().phase.is_active() {
            break;
        }
    }
    let page = worker
        .preview_files(id, &PreviewFilesQuery::default())
        .await
        .unwrap();
    assert!(page.complete);
}

#[tokio::test]
async fn serialized_response_bound_accounts_for_json_escaping() {
    use hmail_cleaner_agent::{
        execution::{NativeExecutor, Worker},
        http,
    };
    use hmail_cleaner_protocol::AgentId;
    use std::sync::Arc;
    use tower::ServiceExt;
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let command: JobCommand =
        serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap();
    let job = validate_job(command).unwrap();
    let id = job.command().job_id;
    repo.accept(&job, repo.fingerprint(&job)).await.unwrap();
    let rows = (0..100)
        .map(|index| PreviewFile {
            id: index.to_string(),
            path: "\u{1}".repeat(4096),
            size_bytes: 0.into(),
            created_at: chrono::DateTime::UNIX_EPOCH,
            orphan: true,
        })
        .collect::<Vec<_>>();
    repo.append_preview_files(id, &rows).await.unwrap();
    let router = http::router(Worker::new(AgentId::new(), repo, NativeExecutor));
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/agent/v1/jobs/{id}/files?limit=100"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn historical_uncaptured_preview_is_gone_while_captured_zero_preview_remains_empty() {
    use hmail_cleaner_agent::{
        execution::{NativeExecutor, Worker},
        http,
        jobs::JobUpdate,
    };
    use hmail_cleaner_protocol::{AgentId, JobPhase, ObservedMailState, SummaryDto};
    use std::sync::Arc;
    use tower::ServiceExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.sqlite");
    let repo = JobRepository::open(&path).await.unwrap();
    let command: JobCommand =
        serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap();
    let job = validate_job(command).unwrap();
    let captured = job.command().job_id;
    repo.accept(&job, repo.fingerprint(&job)).await.unwrap();
    for phase in [JobPhase::Running, JobPhase::Completed] {
        repo.transition(
            captured,
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
    // A prefeature receipt has no admission-created catalog marker.
    let mut legacy = repo.lookup(captured).await.unwrap().unwrap();
    legacy.job_id = JobId::new();
    legacy.summary.selected_files = 3.into();
    let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    sqlx::query("INSERT INTO jobs(job_id,phase,snapshot) VALUES(?,?,?)")
        .bind(legacy.job_id.to_string())
        .bind(legacy.phase.as_str())
        .bind(serde_json::to_string(&legacy).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    repo.close().await;
    let repo = Arc::new(JobRepository::open(&path).await.unwrap());
    let router = http::router(Worker::new(AgentId::new(), repo.clone(), NativeExecutor));
    let legacy_response = router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/agent/v1/jobs/{}/files", legacy.job_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy_response.status(), axum::http::StatusCode::GONE);
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/agent/v1/jobs/{captured}/files"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let page = repo
        .preview_files(captured, &PreviewFilesQuery::default())
        .await
        .unwrap();
    assert_eq!(page.total.get(), 0);
    assert!(page.rows.is_empty());
    assert!(page.complete);
    assert_eq!(
        repo.lookup(legacy.job_id)
            .await
            .unwrap()
            .unwrap()
            .summary
            .selected_files
            .get(),
        3
    );
}

#[tokio::test]
async fn date_and_relation_filters_combine_before_counting_and_paging() {
    let dir = tempfile::tempdir().unwrap();
    let repo = JobRepository::open(&dir.path().join("jobs.sqlite"))
        .await
        .unwrap();
    let command: JobCommand =
        serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap();
    let job = validate_job(command).unwrap();
    let id = job.command().job_id;
    repo.accept(&job, repo.fingerprint(&job)).await.unwrap();
    let rows = [
        ("a", "2026-09-01T10:00:00.123456789Z", false),
        ("b", "2026-09-01T10:00:00.123456790Z", true),
        ("c", "2026-09-02T10:00:00Z", false),
    ]
    .into_iter()
    .map(|(id, created_at, orphan)| PreviewFile {
        id: id.into(),
        path: format!("D:/Mail/{id}.eml"),
        size_bytes: 1.into(),
        created_at: created_at.parse().unwrap(),
        orphan,
    })
    .collect::<Vec<_>>();
    repo.append_preview_files(id, &rows).await.unwrap();
    let query: PreviewFilesQuery = serde_json::from_value(serde_json::json!({
        "created_from": "2026-09-01T13:00:00.123456789+03:00",
        "created_to": "2026-09-01T10:00:00.123456790Z", "orphan": false, "limit": 1,
        "search": "MAIL"
    }))
    .unwrap();
    let page = repo.preview_files(id, &query).await.unwrap();
    assert_eq!(page.total.get(), 1);
    assert_eq!(page.rows[0].id, "a");
    let orphan_query: PreviewFilesQuery = serde_json::from_value(serde_json::json!({
        "created_to": "2026-09-01T10:00:00.123456790Z", "orphan": true
    }))
    .unwrap();
    assert_eq!(
        repo.preview_files(id, &orphan_query).await.unwrap().rows[0].id,
        "b"
    );
    let invalid: PreviewFilesQuery = serde_json::from_value(serde_json::json!({
        "created_from":"2026-09-02T00:00:00Z", "created_to":"2026-09-01T00:00:00Z"
    }))
    .unwrap();
    assert!(invalid.validate().is_err());
}
