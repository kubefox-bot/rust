pub mod fixtures;
use hmail_cleaner_center::{
    account::AccountService,
    vault::{self, RecoveryKey, SecretText},
};
#[tokio::test]
async fn candidate_requires_confirmation_and_old_key_survives_until_commit() {
    let f = fixtures::center().await;
    let service = AccountService::new(f.store.clone(), f.authority.clone());
    let old = RecoveryKey::parse(&f.recovery).unwrap();
    let candidate = service
        .prepare_recovery(&f.lease, SecretText::new("fixture-center-password".into()))
        .await
        .unwrap();
    let new = RecoveryKey::parse(&candidate.recovery_key).unwrap();
    let before = f.store.account().await.unwrap().unwrap();
    assert!(vault::open_recovery(&before.material, &old).is_ok());
    assert!(vault::open_recovery(&before.material, &new).is_err());
    service
        .confirm_recovery(&f.lease, candidate.candidate_id, &new)
        .await
        .unwrap();
    let after = f.store.account().await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&before.material).unwrap()["password_wrap"],
        serde_json::to_value(&after.material).unwrap()["password_wrap"]
    );
    assert!(vault::open_recovery(&after.material, &old).is_err());
    assert!(vault::open_recovery(&after.material, &new).is_ok());
    assert!(vault::open_password(
        &after.material,
        &SecretText::new("fixture-center-password".into())
    )
    .is_ok());
    assert!(service
        .confirm_recovery(&f.lease, candidate.candidate_id, &new)
        .await
        .is_err());
}

#[tokio::test]
async fn candidate_expires_and_logout_invalidates_it() {
    let f = fixtures::center().await;
    let service = AccountService::new(f.store.clone(), f.authority.clone());
    let candidate = service
        .prepare_recovery(&f.lease, SecretText::new("fixture-center-password".into()))
        .await
        .unwrap();
    let new = RecoveryKey::parse(&candidate.recovery_key).unwrap();
    tokio::time::pause();
    tokio::time::advance(std::time::Duration::from_secs(601)).await;
    assert!(service
        .confirm_recovery(&f.lease, candidate.candidate_id, &new)
        .await
        .is_err());
    tokio::time::resume();
    let candidate = service
        .prepare_recovery(&f.lease, SecretText::new("fixture-center-password".into()))
        .await
        .unwrap();
    let new = RecoveryKey::parse(&candidate.recovery_key).unwrap();
    f.authority.revoke(f.lease.generation());
    assert!(service
        .confirm_recovery(&f.lease, candidate.candidate_id, &new)
        .await
        .is_err());
    assert_eq!(f.store.account().await.unwrap().unwrap().revision, 1);
}

#[tokio::test]
async fn replacement_candidate_and_restart_never_reveal_or_accept_previous_candidate() {
    let f = fixtures::center().await;
    let service = AccountService::new(f.store.clone(), f.authority.clone());
    let first = service
        .prepare_recovery(&f.lease, SecretText::new("fixture-center-password".into()))
        .await
        .unwrap();
    let second = service
        .prepare_recovery(&f.lease, SecretText::new("fixture-center-password".into()))
        .await
        .unwrap();
    assert!(service
        .confirm_recovery(
            &f.lease,
            first.candidate_id,
            &RecoveryKey::parse(&first.recovery_key).unwrap()
        )
        .await
        .is_err());
    let restarted = AccountService::new(f.store.clone(), f.authority.clone());
    assert!(restarted
        .confirm_recovery(
            &f.lease,
            second.candidate_id,
            &RecoveryKey::parse(&second.recovery_key).unwrap()
        )
        .await
        .is_err());
    assert_eq!(f.store.account().await.unwrap().unwrap().revision, 1);
}
#[tokio::test]
async fn credential_routes_require_session_csrf_and_never_cache_candidate() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use hmail_cleaner_center::{
        config::BrowserOrigin,
        http::{router, AppState},
    };
    use tower::ServiceExt;
    let f = fixtures::center().await;
    let state = AppState::new(
        f.store.clone(),
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap()
    .with_authority(f.authority.clone());
    let app = router(state);
    let request = |path: &str, csrf: bool| {
        let builder = Request::builder()
            .method("POST")
            .uri(path)
            .header("host", "localhost:8443")
            .header("origin", "https://localhost:8443")
            .header("content-type", "application/json")
            .header(
                "cookie",
                format!("__Host-hmail-session={}", f.token.as_str()),
            );
        let builder = if csrf {
            builder.header("x-hmail-request", "1")
        } else {
            builder
        };
        builder
            .body(Body::from(
                r#"{"current_password":"fixture-center-password"}"#,
            ))
            .unwrap()
    };
    let denied = app
        .clone()
        .oneshot(request("/api/v1/account/recovery/prepare", false))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let response = app
        .clone()
        .oneshot(request("/api/v1/account/recovery/prepare", true))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let bytes = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let candidate: hmail_cleaner_protocol::browser::auth::RecoveryCandidate =
        serde_json::from_slice(&bytes).unwrap();
    let get = Request::builder()
        .uri(format!(
            "/api/v1/account/recovery/{}",
            candidate.candidate_id
        ))
        .header("host", "localhost:8443")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(get).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
}
