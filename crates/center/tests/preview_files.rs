pub mod fixtures;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use hmail_cleaner_center::{
    config::BrowserOrigin,
    http::{router, AppState},
};
use hmail_cleaner_protocol::{
    browser::jobs::SubmitRequest, JobId, JobPhase, Operation, PreviewFilesPage, RunLimits, Scan,
    ServicePolicy,
};
use tower::ServiceExt;

async fn read(app: &Router, uri: &str, token: Option<&str>) -> axum::response::Response {
    let request = Request::builder().uri(uri).header("host", "localhost:8443");
    let request = match token {
        Some(token) => request.header("cookie", format!("__Host-hmail-session={token}")),
        None => request,
    };
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn preview_pages_are_authenticated_job_bound_bounded_and_refresh_during_run() {
    let f = fixtures::center().await;
    let (agent, control) = fixtures::job_agent::start(&f).await;
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
    let id = JobId::new();
    state
        .jobs
        .submit(
            &f.lease,
            SubmitRequest {
                job_id: id,
                server_id: target.server_id,
                profile_version: 2.into(),
                operation: Operation::Preview,
                scan: Scan::Fast,
                resolved_cutoff: fixtures::jobs::cutoff(),
                service_policy: ServicePolicy::Manage,
                limits: RunLimits::default(),
                preview_job_id: None,
                confirmed: false,
            },
        )
        .await
        .unwrap();
    let app = router(state);
    let uri = format!("/api/v1/jobs/{id}/files");
    assert_eq!(
        read(&app, &uri, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let token = Some(f.token.as_str());
    let response = read(&app, &uri, token).await;
    assert_eq!(response.status(), StatusCode::OK);
    let page: PreviewFilesPage =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(page.total.get(), 0);
    assert!(!page.complete);
    control.progress(id, JobPhase::Running);
    let response = read(
        &app,
        &format!("{uri}?limit=10&sort=size&direction=desc"),
        token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let page: PreviewFilesPage =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(page.total.get(), 12);
    assert_eq!(page.rows.len(), 10);
    assert!(page
        .rows
        .windows(2)
        .all(|rows| rows[0].size_bytes >= rows[1].size_bytes));
    let response = read(&app, &format!("{uri}?search=message-0001"), token).await;
    let filtered: PreviewFilesPage =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(filtered.total.get(), 1);
    assert_eq!(filtered.rows.len(), 1);
    let boundary = filtered.rows[0]
        .created_at
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let response = read(
        &app,
        &format!("{uri}?created_from={boundary}&created_to={boundary}&orphan=true"),
        token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let bounded: PreviewFilesPage =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(bounded.total.get(), 1);
    assert_eq!(bounded.rows[0].id, filtered.rows[0].id);
    assert_eq!(
        read(
            &app,
            &format!("{uri}?created_from=2026-09-02T00:00:00Z&created_to=2026-09-01T00:00:00Z"),
            token
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read(&app, &format!("{uri}?limit=101"), token)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read(&app, &format!("{uri}?sort=injected"), token)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read(&app, &format!("/api/v1/jobs/{}/files", JobId::new()), token)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    control.wrong_file_job(true);
    let response = read(&app, &uri, token).await;
    assert!(!response.status().is_success());
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert!(!String::from_utf8_lossy(&body).contains("hmail-mvp-synthetic"));
    control.wrong_file_job(false);
    f.authority.revoke(f.lease.generation());
    assert_eq!(
        read(&app, &uri, token).await.status(),
        StatusCode::UNAUTHORIZED
    );
}
