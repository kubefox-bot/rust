#![cfg(unix)]
use astra_updater::{
    journal::OperationLock,
    reboot::{ClientIdentity, Clock, Coordinator},
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        "2026-10-18T03:30:00Z".parse().unwrap()
    }
}
fn fixture() -> (tempfile::TempDir, ClientIdentity, Arc<Coordinator>) {
    let root = tempfile::tempdir().unwrap();
    std::fs::set_permissions(
        root.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    let client = ClientIdentity {
        id: Uuid::new_v4(),
        group: "astra".into(),
    };
    let coordinator = Arc::new(
        Coordinator::new(
            root.path().into(),
            root.path().join("missing.sock"),
            client.clone(),
            Uuid::new_v4(),
            Arc::new(FixedClock),
        )
        .unwrap(),
    );
    (root, client, coordinator)
}
fn request(client: &ClientIdentity, header: bool) -> Request<Body> {
    let mut request = Request::post("/v1/pre-reboot").header("content-type", "application/json");
    if header {
        request = request.header("fleet-lock-protocol", "true");
    }
    request
        .body(Body::from(
            serde_json::json!({"client_params":client}).to_string(),
        ))
        .unwrap()
}
#[tokio::test]
async fn fleet_lock_rejects_missing_header_and_unknown_client() {
    let (_, client, coordinator) = fixture();
    let app = astra_updater::reboot::router(coordinator);
    assert_eq!(
        app.clone()
            .oneshot(request(&client, false))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let stranger = ClientIdentity {
        id: Uuid::new_v4(),
        group: "astra".into(),
    };
    assert_eq!(
        app.oneshot(request(&stranger, true))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
}
#[tokio::test]
async fn missing_center_or_busy_updater_never_grants_reboot() {
    let (root, client, coordinator) = fixture();
    let app = astra_updater::reboot::router(coordinator);
    assert_eq!(
        app.clone()
            .oneshot(request(&client, true))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(!root.path().join("reboot-grant.json").exists());
    let _updater = OperationLock::acquire(&root.path().join("operation.lock")).unwrap();
    assert_eq!(
        app.oneshot(request(&client, true)).await.unwrap().status(),
        StatusCode::LOCKED
    );
}

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixListener,
    sync::Mutex,
};
struct ControlState {
    owner: Option<Uuid>,
    jobs: u64,
    enrollments: u64,
    writers: u64,
    malformed: bool,
}
struct Control {
    state: Arc<Mutex<ControlState>>,
    task: tokio::task::AbortHandle,
}
impl Drop for Control {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn control(path: &std::path::Path) -> Control {
    let listener = UnixListener::bind(path).unwrap();
    let state = Arc::new(Mutex::new(ControlState {
        owner: None,
        jobs: 0,
        enrollments: 0,
        writers: 0,
        malformed: false,
    }));
    let shared = state.clone();
    let task = tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let (reader, mut writer) = stream.into_split();
            let mut line = String::new();
            BufReader::new(reader).read_line(&mut line).await.unwrap();
            let input: serde_json::Value = serde_json::from_str(&line).unwrap();
            let id = input["id"].as_str().map(|id| Uuid::parse_str(id).unwrap());
            let mut state = shared.lock().await;
            let output = if state.malformed {
                "{\"version\":1,\"status\":\"unknown\"}".into()
            } else {
                let mut conflict = false;
                match input["operation"].as_str().unwrap() {
                    "begin" => {
                        if state.owner.is_some() && state.owner != id {
                            conflict = true;
                        } else {
                            state.owner = id;
                        }
                    }
                    "finish" => {
                        if state.owner.is_some() && state.owner != id {
                            conflict = true;
                        } else {
                            state.owner = None;
                        }
                    }
                    "policy" | "status" => (),
                    _ => panic!("unexpected control operation"),
                }
                if conflict {
                    "{\"version\":1,\"error\":\"conflict\"}".into()
                } else {
                    serde_json::json!({"version":1,"status":if state.owner.is_none(){"idle"}else if state.jobs+state.enrollments+state.writers==0{"ready"}else{"draining"},"active_jobs":state.jobs,"active_enrollments":state.enrollments,"active_writers":state.writers,"policy":{"check_interval_seconds":60,"graceful_timeout_minutes":15}}).to_string()
                }
            };
            writer
                .write_all(format!("{output}\n").as_bytes())
                .await
                .unwrap();
        }
    });
    Control {
        state,
        task: task.abort_handle(),
    }
}
fn steady(client: &ClientIdentity) -> Request<Body> {
    let mut request = request(client, true);
    *request.uri_mut() = "/v1/steady-state".parse().unwrap();
    request
}
fn coordinator(
    root: &std::path::Path,
    client: &ClientIdentity,
    boot: Uuid,
    clock: Arc<dyn Clock>,
) -> Arc<Coordinator> {
    Arc::new(
        Coordinator::new(
            root.into(),
            root.join("control.sock"),
            client.clone(),
            boot,
            clock,
        )
        .unwrap(),
    )
}

#[tokio::test]
async fn idle_grant_is_durable_recursive_and_holds_both_locks_until_steady_state() {
    let (root, client, _) = fixture();
    let control = control(&root.path().join("control.sock")).await;
    let app = astra_updater::reboot::router(coordinator(
        root.path(),
        &client,
        Uuid::new_v4(),
        Arc::new(FixedClock),
    ));
    assert_eq!(
        app.clone()
            .oneshot(request(&client, true))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let first = std::fs::read(root.path().join("reboot-grant.json")).unwrap();
    let grant: serde_json::Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(grant["phase"], "granted");
    assert!(OperationLock::acquire(&root.path().join("operation.lock")).is_err());
    assert_eq!(
        app.clone()
            .oneshot(request(&client, true))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        std::fs::read(root.path().join("reboot-grant.json")).unwrap(),
        first
    );
    assert_eq!(
        app.oneshot(steady(&client)).await.unwrap().status(),
        StatusCode::OK
    );
    assert!(!root.path().join("reboot-grant.json").exists());
    assert!(control.state.lock().await.owner.is_none());
    assert!(OperationLock::acquire(&root.path().join("operation.lock")).is_ok());
}

#[tokio::test]
async fn active_jobs_enrollments_or_writers_defer_then_idle_can_grant() {
    for slot in ["jobs", "enrollments", "writers"] {
        let (root, client, _) = fixture();
        let control = control(&root.path().join("control.sock")).await;
        {
            let mut state = control.state.lock().await;
            match slot {
                "jobs" => state.jobs = 1,
                "enrollments" => state.enrollments = 1,
                "writers" => state.writers = 1,
                _ => unreachable!(),
            }
        }
        let app = astra_updater::reboot::router(coordinator(
            root.path(),
            &client,
            Uuid::new_v4(),
            Arc::new(FixedClock),
        ));
        assert_eq!(
            app.clone()
                .oneshot(request(&client, true))
                .await
                .unwrap()
                .status(),
            StatusCode::LOCKED
        );
        let grant: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.path().join("reboot-grant.json")).unwrap())
                .unwrap();
        assert_eq!(grant["phase"], "pending");
        {
            let mut state = control.state.lock().await;
            state.jobs = 0;
            state.enrollments = 0;
            state.writers = 0;
        }
        assert_eq!(
            app.oneshot(request(&client, true)).await.unwrap().status(),
            StatusCode::OK
        );
    }
}
struct OutsideWindow;
impl Clock for OutsideWindow {
    fn now(&self) -> DateTime<Utc> {
        "2026-10-18T04:00:00Z".parse().unwrap()
    }
}
#[tokio::test]
async fn outside_window_and_unknown_center_never_create_grants_or_claim_other_marker() {
    let (root, client, _) = fixture();
    let control = control(&root.path().join("control.sock")).await;
    let app = astra_updater::reboot::router(coordinator(
        root.path(),
        &client,
        Uuid::new_v4(),
        Arc::new(OutsideWindow),
    ));
    assert_eq!(
        app.oneshot(request(&client, true)).await.unwrap().status(),
        StatusCode::LOCKED
    );
    assert!(!root.path().join("reboot-grant.json").exists());
    let app = astra_updater::reboot::router(coordinator(
        root.path(),
        &client,
        Uuid::new_v4(),
        Arc::new(FixedClock),
    ));
    control.state.lock().await.malformed = true;
    assert_eq!(
        app.clone()
            .oneshot(request(&client, true))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(!root.path().join("reboot-grant.json").exists());
    let other = Uuid::new_v4();
    {
        let mut state = control.state.lock().await;
        state.malformed = false;
        state.owner = Some(other);
    }
    assert_eq!(
        app.oneshot(request(&client, true)).await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(control.state.lock().await.owner, Some(other));
    assert!(!root.path().join("reboot-grant.json").exists());
}
#[tokio::test]
async fn a_new_boot_requires_steady_state_recovery_and_never_unlocks_another_operation() {
    let (root, client, _) = fixture();
    let control = control(&root.path().join("control.sock")).await;
    let boot = Uuid::new_v4();
    let app = astra_updater::reboot::router(coordinator(
        root.path(),
        &client,
        boot,
        Arc::new(FixedClock),
    ));
    assert_eq!(
        app.oneshot(request(&client, true)).await.unwrap().status(),
        StatusCode::OK
    );
    // The last router owner was consumed above; process restart releases flock but not the journal.
    assert!(OperationLock::acquire(&root.path().join("operation.lock")).is_ok());
    let restarted = astra_updater::reboot::router(coordinator(
        root.path(),
        &client,
        Uuid::new_v4(),
        Arc::new(FixedClock),
    ));
    assert_eq!(
        restarted
            .clone()
            .oneshot(request(&client, true))
            .await
            .unwrap()
            .status(),
        StatusCode::LOCKED
    );
    let owner = control.state.lock().await.owner.unwrap();
    let other = Uuid::new_v4();
    control.state.lock().await.owner = Some(other);
    assert_eq!(
        restarted
            .clone()
            .oneshot(steady(&client))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(control.state.lock().await.owner, Some(other));
    assert!(root.path().join("reboot-grant.json").exists());
    control.state.lock().await.owner = Some(owner);
    assert_eq!(
        restarted.oneshot(steady(&client)).await.unwrap().status(),
        StatusCode::OK
    );
    assert!(!root.path().join("reboot-grant.json").exists());
}
#[tokio::test]
async fn persistence_failure_cannot_acknowledge_reboot_or_drain_center() {
    let (root, client, _) = fixture();
    let control = control(&root.path().join("control.sock")).await;
    let app = astra_updater::reboot::router(coordinator(
        root.path(),
        &client,
        Uuid::new_v4(),
        Arc::new(FixedClock),
    ));
    std::fs::create_dir(root.path().join("reboot-grant.json")).unwrap();
    assert_eq!(
        app.oneshot(request(&client, true)).await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(control.state.lock().await.owner.is_none());
    assert!(OperationLock::acquire(&root.path().join("operation.lock")).is_ok());
}

struct MovingClock(std::sync::atomic::AtomicI64);
impl Clock for MovingClock {
    fn now(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.0.load(std::sync::atomic::Ordering::SeqCst), 0).unwrap()
    }
}
#[tokio::test]
async fn pending_grant_expires_with_window_and_releases_only_its_own_admission() {
    let (root, client, _) = fixture();
    let control = control(&root.path().join("control.sock")).await;
    control.state.lock().await.jobs = 1;
    let now: DateTime<Utc> = "2026-10-18T03:59:59Z".parse().unwrap();
    let clock = Arc::new(MovingClock(std::sync::atomic::AtomicI64::new(
        now.timestamp(),
    )));
    let app = astra_updater::reboot::router(coordinator(
        root.path(),
        &client,
        Uuid::new_v4(),
        clock.clone(),
    ));
    assert_eq!(
        app.clone()
            .oneshot(request(&client, true))
            .await
            .unwrap()
            .status(),
        StatusCode::LOCKED
    );
    assert!(root.path().join("reboot-grant.json").exists());
    clock
        .0
        .store(now.timestamp() + 1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        app.oneshot(request(&client, true)).await.unwrap().status(),
        StatusCode::LOCKED
    );
    assert!(!root.path().join("reboot-grant.json").exists());
    assert!(control.state.lock().await.owner.is_none());
    assert!(OperationLock::acquire(&root.path().join("operation.lock")).is_ok());
}
#[tokio::test]
async fn malformed_or_oversized_fleet_lock_body_cannot_request_admission() {
    let (root, client, coordinator) = fixture();
    let app = astra_updater::reboot::router(coordinator);
    for payload in [
        serde_json::json!({"client_params":client,"exec":"reboot"}).to_string(),
        " ".repeat(1025),
    ] {
        let input = Request::post("/v1/pre-reboot")
            .header("content-type", "application/json")
            .header("fleet-lock-protocol", "true")
            .body(Body::from(payload))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(input).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert!(!root.path().join("reboot-grant.json").exists());
}
#[test]
fn corrupt_grant_journal_is_not_ignored_on_coordinator_restart() {
    let (root, client, _) = fixture();
    std::fs::write(root.path().join("reboot-grant.json"), b"broken").unwrap();
    assert!(Coordinator::new(
        root.path().into(),
        root.path().join("control.sock"),
        client,
        Uuid::new_v4(),
        Arc::new(FixedClock)
    )
    .is_err());
}

#[tokio::test]
async fn fleet_lock_identity_is_case_sensitive_and_not_uuid_normalized() {
    let (root, _, _) = fixture();
    let client = ClientIdentity {
        id: "abcdefab-cdef-4abc-8abc-abcdefabcdef".parse().unwrap(),
        group: "astra".into(),
    };
    let coordinator = coordinator(root.path(), &client, Uuid::new_v4(), Arc::new(FixedClock));
    let input=Request::post("/v1/pre-reboot").header("content-type","application/json").header("fleet-lock-protocol","true").body(Body::from(serde_json::json!({"client_params":{"id":client.id.to_string().to_uppercase(),"group":client.group}}).to_string())).unwrap();
    assert_eq!(
        astra_updater::reboot::router(coordinator)
            .oneshot(input)
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn reboot_window_excludes_other_days_and_exact_boundary_seconds() {
    for timestamp in [
        "2026-10-17T03:30:00Z",
        "2026-10-18T02:59:59Z",
        "2026-10-18T04:00:00Z",
        "2026-10-19T03:30:00Z",
    ] {
        let (root, client, _) = fixture();
        let now: DateTime<Utc> = timestamp.parse().unwrap();
        let clock = Arc::new(MovingClock(std::sync::atomic::AtomicI64::new(
            now.timestamp(),
        )));
        let app =
            astra_updater::reboot::router(coordinator(root.path(), &client, Uuid::new_v4(), clock));
        assert_eq!(
            app.oneshot(request(&client, true)).await.unwrap().status(),
            StatusCode::LOCKED
        );
        assert!(!root.path().join("reboot-grant.json").exists());
    }
}
