pub mod fixtures;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use hmail_cleaner_center::{
    config::BrowserOrigin,
    http::{router, AppState},
};
use hmail_cleaner_protocol::browser::jobs::CleanupContext;
use tower::ServiceExt;
#[tokio::test]
async fn cleanup_context_is_authenticated_bounded_and_contains_no_technical_profile() {
    let f = fixtures::center().await;
    let (agent, _) = fixtures::job_agent::start(&f).await;
    let target = fixtures::jobs::configured(
        &f,
        agent.credentials.agent_id,
        agent.credentials.endpoint.as_str(),
    )
    .await;
    let state = AppState::new(
        f.store.clone(),
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap()
    .with_authority(f.authority.clone());
    let app = router(state);
    let url = format!("/api/v1/servers/{}/cleanup-context", target.server_id);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&url)
                .header("host", "localhost:8443")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .oneshot(
            Request::builder()
                .uri(url)
                .header("host", "localhost:8443")
                .header(
                    "cookie",
                    format!("__Host-hmail-session={}", f.token.as_str()),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 16384).await.unwrap();
    let context: CleanupContext = serde_json::from_slice(&bytes).unwrap();
    assert!(context.configured);
    assert_eq!(context.profile_version.get(), 2);
    assert!(context.latest.is_none());
    assert!(!context.busy);
    let text = std::str::from_utf8(&bytes).unwrap();
    for forbidden in [
        "mysql_host",
        "data_dir",
        "hmail_ini",
        "NEVER-PERSIST-JOB-PASSWORD",
    ] {
        assert!(!text.contains(forbidden));
    }
}
