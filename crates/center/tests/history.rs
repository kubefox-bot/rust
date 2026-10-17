pub mod fixtures;
use hmail_cleaner_center::jobs::JobService;
use hmail_cleaner_protocol::{
    browser::jobs::SubmitRequest, JobId, Operation, RunLimits, Scan, ServerId, ServicePolicy,
};
#[tokio::test]
async fn history_pagination_is_bounded_stable_and_separates_server_targets() {
    let center = fixtures::center().await;
    let target = fixtures::jobs::register(&center, "https://127.0.0.1:9").await;
    let other = fixtures::jobs::register(&center, "https://127.0.0.1:10").await;
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        center.root.path().join("center.sqlite").display()
    ))
    .await
    .unwrap();
    let mut ids = Vec::new();
    for server in [
        target.server_id,
        target.server_id,
        other.server_id,
        target.server_id,
    ] {
        let id = JobId::new();
        ids.push(id);
        let profile = if server == target.server_id {
            target.profile_id
        } else {
            other.profile_id
        };
        let input = SubmitRequest {
            job_id: id,
            server_id: server,
            profile_version: 2.into(),
            operation: Operation::Preview,
            scan: Scan::Fast,
            resolved_cutoff: fixtures::jobs::cutoff(),
            service_policy: ServicePolicy::Manage,
            limits: RunLimits::default(),
            preview_job_id: None,
            confirmed: false,
        };
        sqlx::query("INSERT INTO jobs(id,server_id,profile_id,profile_version,phase,intent) VALUES(?,?,?,2,'unknown',?)").bind(id.to_string()).bind(server.to_string()).bind(profile.to_string()).bind(serde_json::to_vec(&input).unwrap()).execute(&pool).await.unwrap();
    }
    let jobs = JobService::new(center.store.clone());
    let page = jobs
        .history(target.server_id, None, 2, &center.lease)
        .await
        .unwrap();
    assert_eq!(
        page.jobs.iter().map(|j| j.job_id).collect::<Vec<_>>(),
        vec![ids[3], ids[1]]
    );
    let last = jobs
        .history(target.server_id, page.next_cursor, 2, &center.lease)
        .await
        .unwrap();
    assert_eq!(last.jobs.len(), 1);
    assert_eq!(last.jobs[0].job_id, ids[0]);
    assert!(last.next_cursor.is_none());
    assert!(jobs
        .history(ServerId::new(), None, 50, &center.lease)
        .await
        .unwrap()
        .jobs
        .is_empty());
    assert!(jobs
        .history(target.server_id, None, 0, &center.lease)
        .await
        .is_err());
    pool.close().await;
    for name in ["center.sqlite", "center.sqlite-wal"] {
        if let Ok(bytes) = std::fs::read(center.root.path().join(name)) {
            assert!(!bytes
                .windows(26)
                .any(|part| part == b"NEVER-PERSIST-JOB-PASSWORD"));
        }
    }
}
