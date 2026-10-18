#[allow(dead_code)]
mod fixtures;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use hmail_cleaner_center::{
    config::BrowserOrigin,
    http::{router, AppState},
};
use tower::ServiceExt;

async fn app(f: &fixtures::CenterFixture) -> Router {
    router(
        AppState::new(
            f.store.clone(),
            BrowserOrigin::parse("https://localhost:8443").unwrap(),
        )
        .await
        .unwrap()
        .with_authority(f.authority.clone()),
    )
}
fn request(method: &str, token: Option<&str>, body: &str, csrf: bool) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri("/api/v1/settings/updates")
        .header("host", "localhost:8443")
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("cookie", format!("__Host-hmail-session={token}"));
    }
    if csrf {
        request = request.header("x-hmail-request", "1");
    }
    request.body(Body::from(body.to_owned())).unwrap()
}
async fn json(response: axum::response::Response) -> serde_json::Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap()
}
#[tokio::test]
async fn authenticated_settings_default_then_persist_across_router_restart() {
    let f = fixtures::center().await;
    let response = app(&f)
        .await
        .oneshot(request("GET", Some(&f.token), "", false))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json(response).await,
        serde_json::json!({"check_interval_seconds":60,"graceful_timeout_minutes":15})
    );
    let response = app(&f)
        .await
        .oneshot(request(
            "PATCH",
            Some(&f.token),
            r#"{"check_interval_seconds":120,"graceful_timeout_minutes":30}"#,
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app(&f)
        .await
        .oneshot(request("GET", Some(&f.token), "", false))
        .await
        .unwrap();
    assert_eq!(
        json(response).await,
        serde_json::json!({"check_interval_seconds":120,"graceful_timeout_minutes":30})
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(f.root.path().join("update-policy.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
#[tokio::test]
async fn policy_requires_session_and_csrf_and_rejects_invalid_values_without_writing() {
    let f = fixtures::center().await;
    let app = app(&f).await;
    assert_eq!(
        app.clone()
            .oneshot(request("GET", None, "", false))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                "PATCH",
                Some(&f.token),
                r#"{"check_interval_seconds":60,"graceful_timeout_minutes":15}"#,
                false
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    for payload in [
        r#"{"check_interval_seconds":29,"graceful_timeout_minutes":15}"#,
        r#"{"check_interval_seconds":86401,"graceful_timeout_minutes":15}"#,
        r#"{"check_interval_seconds":60,"graceful_timeout_minutes":0}"#,
        r#"{"check_interval_seconds":60,"graceful_timeout_minutes":1441}"#,
        r#"{"check_interval_seconds":60,"graceful_timeout_minutes":15,"disable_auto":true}"#,
    ] {
        assert_eq!(
            app.clone()
                .oneshot(request("PATCH", Some(&f.token), payload, true))
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert!(!f.root.path().join("update-policy.json").exists());
}
#[tokio::test]
async fn corrupt_policy_fails_closed_instead_of_returning_defaults() {
    let f = fixtures::center().await;
    std::fs::write(f.root.path().join("update-policy.json"), b"broken-json").unwrap();
    assert_eq!(
        app(&f)
            .await
            .oneshot(request("GET", Some(&f.token), "", false))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn update_policy_is_readable_without_vault_and_corrupt_metadata_is_never_defaulted() {
    use hmail_cleaner_center::update_settings::read_policy;
    let root = tempfile::tempdir().unwrap();
    let defaults = read_policy(root.path()).unwrap();
    assert_eq!(
        (
            defaults.check_interval_seconds,
            defaults.graceful_timeout_minutes
        ),
        (60, 15)
    );
    for contents in [
        r#"{"format_version":2,"policy":{"check_interval_seconds":60,"graceful_timeout_minutes":15}}"#,
        r#"{"format_version":1,"policy":{"check_interval_seconds":0,"graceful_timeout_minutes":15}}"#,
        r#"{"format_version":1,"policy":{"check_interval_seconds":60}}"#,
    ] {
        std::fs::write(root.path().join("update-policy.json"), contents).unwrap();
        assert!(read_policy(root.path()).is_err());
    }
    std::fs::write(root.path().join("update-policy.json"), vec![b' '; 4097]).unwrap();
    assert!(read_policy(root.path()).is_err());
}

#[tokio::test]
async fn update_policy_changes_are_refused_during_drain() {
    let f = fixtures::center().await;
    f.store.maintenance().begin(uuid::Uuid::new_v4()).unwrap();
    let response = app(&f)
        .await
        .oneshot(request(
            "PATCH",
            Some(&f.token),
            r#"{"check_interval_seconds":120,"graceful_timeout_minutes":30}"#,
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!f.root.path().join("update-policy.json").exists());
}

#[tokio::test]
async fn successful_policy_save_counts_as_activity_but_failed_persistence_does_not() {
    use hmail_cleaner_center::{
        auth::{Clock, SessionAuthority},
        vault::Dek,
    };
    use std::{
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc,
        },
        time::Duration,
    };
    struct ManualClock(AtomicU64);
    impl Clock for ManualClock {
        fn now(&self) -> Duration {
            Duration::from_secs(self.0.load(Ordering::SeqCst))
        }
    }
    let f = fixtures::center().await;
    let clock = Arc::new(ManualClock(AtomicU64::new(0)));
    let authority = Arc::new(SessionAuthority::new(1, 60, clock.clone()).unwrap());
    let grant = authority.admit(1, Dek::new([1; 32])).unwrap();
    let state = AppState::new(
        f.store.clone(),
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap()
    .with_authority(authority.clone());
    let app = router(state);
    clock.0.store(1000, Ordering::SeqCst);
    authority.touch(grant.generation).unwrap();
    clock.0.store(1010, Ordering::SeqCst);
    assert_eq!(
        app.clone()
            .oneshot(request(
                "PATCH",
                Some(&grant.token),
                r#"{"check_interval_seconds":120,"graceful_timeout_minutes":30}"#,
                true
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    clock.0.store(4609, Ordering::SeqCst);
    assert!(authority.authenticate(&grant.token).is_ok());
    std::fs::remove_file(f.root.path().join("update-policy.json")).unwrap();
    std::fs::create_dir(f.root.path().join("update-policy.json")).unwrap();
    assert_eq!(
        app.oneshot(request(
            "PATCH",
            Some(&grant.token),
            r#"{"check_interval_seconds":300,"graceful_timeout_minutes":60}"#,
            true
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    clock.0.store(4610, Ordering::SeqCst);
    assert!(authority.authenticate(&grant.token).is_err());
    assert!(!std::fs::read_dir(f.root.path()).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".update-policy-")));
}
