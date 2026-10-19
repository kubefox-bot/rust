use hmail_cleaner_center::{
    maintenance::{MaintenanceError, MaintenanceStatus},
    storage::{CenterStore, DataDirectory},
};
use uuid::Uuid;

#[tokio::test]
async fn drain_excludes_new_admission_and_waits_for_owned_writers() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    let store = CenterStore::open(&lease).await.unwrap();
    let writer = store.maintenance().admit().unwrap();
    let id = Uuid::new_v4();
    store.maintenance().begin(id).unwrap();
    assert!(matches!(
        store.maintenance().admit(),
        Err(MaintenanceError::Conflict)
    ));
    assert_eq!(
        store.maintenance().status(&store).await.unwrap().status,
        MaintenanceStatus::Draining
    );
    drop(writer);
    assert_eq!(
        store.maintenance().status(&store).await.unwrap().status,
        MaintenanceStatus::Ready
    );
    assert!(store.maintenance().admit().is_err());
    assert!(store.maintenance().end(Uuid::new_v4()).is_err());
    store.maintenance().end(id).unwrap();
    assert!(store.maintenance().admit().is_ok());
}

#[tokio::test]
async fn durable_marker_survives_restart_and_corruption_fails_closed() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    let id = Uuid::new_v4();
    let store = CenterStore::open(&lease).await.unwrap();
    store.maintenance().begin(id).unwrap();
    drop(store);
    let reopened = CenterStore::open(&lease).await.unwrap();
    assert!(reopened.maintenance().admit().is_err());
    reopened.maintenance().begin(id).unwrap();
    assert!(reopened.maintenance().begin(Uuid::new_v4()).is_err());
    reopened.maintenance().end(id).unwrap();
    drop(reopened);
    std::fs::write(root.path().join("maintenance.json"), b"broken").unwrap();
    assert!(CenterStore::open(&lease).await.is_err());
}

#[tokio::test]
async fn uncertain_durable_remote_slot_never_reports_ready() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    let store = CenterStore::open(&lease).await.unwrap();
    // The slot is authoritative even if the corresponding receipt is unknown.
    // Disable FK only on a dedicated connection to build a deliberately inconsistent fixture.
    let mut connection = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        root.path().join("center.sqlite").display()
    ))
    .await
    .unwrap();
    use sqlx::Connection;
    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&mut connection)
        .await
        .unwrap();
    sqlx::query("UPDATE job_slot SET job_id='unknown' WHERE singleton=1")
        .execute(&mut connection)
        .await
        .unwrap();
    store.maintenance().begin(Uuid::new_v4()).unwrap();
    let status = store.maintenance().status(&store).await.unwrap();
    assert_eq!(status.status, MaintenanceStatus::Draining);
    assert_eq!(status.active_jobs, 1);
}

#[tokio::test]
async fn offline_backup_retains_update_policy_and_excludes_transient_marker() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(&root.path().join("data")).unwrap();
    let store = CenterStore::open(&lease).await.unwrap();
    let policy = br#"{"check_interval_seconds":90,"graceful_timeout_minutes":20}"#;
    std::fs::write(root.path().join("data/update-policy.json"), policy).unwrap();
    store.maintenance().begin(Uuid::new_v4()).unwrap();
    let backup = root.path().join("backup");
    store.backup(&backup).await.unwrap();
    assert_eq!(
        std::fs::read(backup.join("update-policy.json")).unwrap(),
        policy
    );
    assert!(!backup.join("maintenance.json").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for entry in std::fs::read_dir(&backup).unwrap() {
            assert_eq!(
                entry.unwrap().metadata().unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}

#[allow(dead_code)]
mod fixtures;
#[tokio::test]
async fn domain_admission_rejects_new_work_but_reconcile_remains_available_while_draining() {
    use hmail_cleaner_center::{
        identity::IdentityService,
        jobs::{JobService, SubmitError},
        profiles::{ProfileService, ServerRegistration},
    };
    use hmail_cleaner_protocol::{AgentId, JobId, ProfileId, ServerId};
    let center = fixtures::center().await;
    let writer = center.store.maintenance().admit().unwrap();
    center.store.maintenance().begin(Uuid::new_v4()).unwrap();
    let id = AgentId::new();
    let (_, csr) = fixtures::csr();
    assert!(IdentityService::new(center.store.clone())
        .issue_agent(id, "127.0.0.1", &csr, &center.lease)
        .await
        .is_err());
    assert!(ProfileService::new(center.store.clone())
        .register(
            ServerRegistration {
                server_id: ServerId::new(),
                profile_id: ProfileId::new(),
                agent_id: id,
                name: "mail".into(),
                endpoint: "https://127.0.0.1:9443".into()
            },
            &center.lease
        )
        .await
        .is_err());
    let jobs = JobService::new(center.store.clone());
    assert!(matches!(
        jobs.reconcile(JobId::new(), &center.lease).await,
        Err(SubmitError::NotFound)
    ));
    drop(writer);
    assert_eq!(
        center
            .store
            .maintenance()
            .status(&center.store)
            .await
            .unwrap()
            .status,
        MaintenanceStatus::Ready
    );
    assert!(matches!(
        jobs.reconcile(JobId::new(), &center.lease).await,
        Err(SubmitError::Busy)
    ));
}

#[tokio::test]
async fn browser_login_and_reconciliation_remain_possible_only_before_ready_seal() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use hmail_cleaner_center::{
        config::BrowserOrigin,
        http::{router, AppState},
    };
    use tower::ServiceExt;
    let center = fixtures::center().await;
    let app = router(
        AppState::new(
            center.store.clone(),
            BrowserOrigin::parse("https://localhost:8443").unwrap(),
        )
        .await
        .unwrap()
        .with_authority(center.authority.clone()),
    );
    let writer = center.store.maintenance().admit().unwrap();
    center.store.maintenance().begin(Uuid::new_v4()).unwrap();
    let request = |path: String, body: &'static str| {
        Request::builder()
            .method("POST")
            .uri(path)
            .header("host", "localhost:8443")
            .header("content-type", "application/json")
            .header("x-hmail-request", "1")
            .body(Body::from(body))
            .unwrap()
    };
    // Invalid credentials reach authentication during drain; ordinary mutations are rejected first.
    assert_eq!(
        app.clone()
            .oneshot(request(
                "/api/v1/auth/login".into(),
                r#"{"login":"admin","password":"wrong"}"#
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.clone()
            .oneshot(request("/api/v1/enrollment/install".into(), "{}"))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let path = format!("/api/v1/jobs/{}/reconcile", Uuid::new_v4());
    assert_eq!(
        app.clone()
            .oneshot(request(path, "{}"))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    drop(writer);
    assert_eq!(
        center
            .store
            .maintenance()
            .status(&center.store)
            .await
            .unwrap()
            .status,
        MaintenanceStatus::Ready
    );
    assert_eq!(
        app.oneshot(request(
            "/api/v1/auth/login".into(),
            r#"{"login":"admin","password":"wrong"}"#
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_marker_is_rejected_without_reading_or_changing_its_target() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(&root.path().join("data")).unwrap();
    let target = root.path().join("unrelated");
    std::fs::write(&target, b"unchanged").unwrap();
    std::os::unix::fs::symlink(&target, lease.path().join("maintenance.json")).unwrap();
    assert!(CenterStore::open(&lease).await.is_err());
    assert_eq!(std::fs::read(target).unwrap(), b"unchanged");
}
