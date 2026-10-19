pub mod fixtures;
use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use hmail_cleaner_center::{
    config::BrowserOrigin,
    http::{router, AppState},
    profiles::{ProfileService, ServerRegistration},
};
use hmail_cleaner_protocol::{browser::agents::AgentView, AgentId, ProfileId, ServerId};
use std::sync::atomic::Ordering;
use tower::ServiceExt;
#[tokio::test]
async fn authorized_reads_coalesce_probes_and_preserve_last_observation_after_revocation() {
    let fixture = fixtures::center().await;
    let agent = fixtures::agents::start(&fixture, AgentId::new()).await;
    ProfileService::new(fixture.store.clone())
        .register(
            ServerRegistration {
                server_id: ServerId::new(),
                profile_id: ProfileId::new(),
                agent_id: agent.credentials.agent_id,
                name: "Mail One".into(),
                endpoint: agent.credentials.endpoint.as_str().into(),
            },
            &fixture.lease,
        )
        .await
        .unwrap();
    let mut state = AppState::new(
        fixture.store.clone(),
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap();
    state = state.with_authority(fixture.authority.clone());
    let app = router(state);
    let cookie = format!("__Host-hmail-session={}", fixture.token.as_str());
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/agents")
                .header("host", "localhost:8443")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 401);
    assert_eq!(agent.health_calls.load(Ordering::SeqCst), 0);
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let response = app
                .clone()
                .oneshot(
                    Request::get("/api/v1/agents")
                        .header("host", "localhost:8443")
                        .header("cookie", &cookie)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 200);
            let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
            let views: Vec<AgentView> = serde_json::from_slice(&bytes).unwrap();
            if views[0].health.healthy == Some(true) {
                assert!(!views[0].health.stale);
                assert!(views[0].health.observed_at.is_some());
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(agent.health_calls.load(Ordering::SeqCst), 1);
    use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection};
    let mut database = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(fixture.root.path().join("center.sqlite")),
    )
    .await
    .unwrap();
    let expiry: i64 = sqlx::query_scalar("SELECT expires_at FROM trust WHERE agent_id=?")
        .bind(agent.credentials.agent_id.to_string())
        .fetch_one(&mut database)
        .await
        .unwrap();
    sqlx::query("UPDATE trust SET expires_at=0 WHERE agent_id=?")
        .bind(agent.credentials.agent_id.to_string())
        .execute(&mut database)
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/agents")
                .header("host", "localhost:8443")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let expired: Vec<AgentView> = serde_json::from_slice(&bytes).unwrap();
    assert!(expired[0].certificate_expired);
    assert!(expired[0].health.stale);
    assert_eq!(expired[0].health.healthy, None);
    sqlx::query("UPDATE trust SET expires_at=? WHERE agent_id=?")
        .bind(expiry)
        .bind(agent.credentials.agent_id.to_string())
        .execute(&mut database)
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/v1/agents/{}/revoke",
                agent.credentials.agent_id
            ))
            .header("host", "localhost:8443")
            .header("cookie", &cookie)
            .header("x-hmail-request", "1")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 204);
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/agents")
                .header("host", "localhost:8443")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let views: Vec<AgentView> = serde_json::from_slice(&bytes).unwrap();
    assert!(views[0].revoked);
    assert!(views[0].health.stale);
    assert_eq!(views[0].health.healthy, None);
    assert!(views[0].health.observed_at.is_some());
    fixture.authority.revoke(fixture.lease.generation());
    let response = app
        .oneshot(
            Request::get("/api/v1/agents")
                .header("host", "localhost:8443")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 401);
}
