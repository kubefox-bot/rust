pub mod fixtures;
use hmail_cleaner_center::{
    agents::{AgentClient, AgentCredentials},
    identity::IdentityService,
};
use hmail_cleaner_protocol::{AgentId, DatabaseParts, DatabaseTls, MailProfile};
use std::sync::atomic::Ordering;
fn mail() -> MailProfile {
    MailProfile {
        mysql: DatabaseParts {
            host: "127.0.0.1".into(),
            port: 3306,
            database: "hmail".into(),
            username: "fixture".into(),
            password: zeroize::Zeroizing::new("PRIVATE-PROFILE-SENTINEL".into()),
            tls: DatabaseTls::DisabledLocal,
        },
        data_dir: r"C:\Mail Data\Данные".into(),
        hmail_ini: r"C:\Mail\hMailServer.ini".into(),
        service_name: "hMailServer".into(),
    }
}
#[tokio::test]
async fn wrong_agent_root_and_revoked_trust_receive_no_profile_bytes() {
    let first = fixtures::center().await;
    let second = fixtures::center().await;
    let server = fixtures::agents::start(&first, AgentId::new()).await;
    let legitimate = AgentClient::new(&first.store, server.credentials.clone(), &first.lease)
        .await
        .unwrap();
    assert!(legitimate.validate_profile(&mail()).await.is_ok());
    let baseline = server.received.load(Ordering::SeqCst);
    assert!(baseline > 0);
    let other_id = AgentId::new();
    let (_, csr) = fixtures::csr();
    IdentityService::new(first.store.clone())
        .issue_agent(other_id, "127.0.0.1", &csr, &first.lease)
        .await
        .unwrap();
    let wrong = AgentClient::new(
        &first.store,
        AgentCredentials {
            agent_id: other_id,
            ..server.credentials.clone()
        },
        &first.lease,
    )
    .await
    .unwrap();
    assert!(wrong.validate_profile(&mail()).await.is_err());
    assert_eq!(server.received.load(Ordering::SeqCst), baseline);
    IdentityService::new(second.store.clone())
        .issue_agent(
            server.credentials.agent_id,
            "127.0.0.1",
            &csr,
            &second.lease,
        )
        .await
        .unwrap();
    let wrong_root = AgentClient::new(
        &second.store,
        AgentCredentials {
            center_id: second
                .store
                .account()
                .await
                .unwrap()
                .unwrap()
                .material
                .vault_id(),
            ..server.credentials.clone()
        },
        &second.lease,
    )
    .await
    .unwrap();
    assert!(wrong_root.validate_profile(&mail()).await.is_err());
    assert_eq!(server.received.load(Ordering::SeqCst), baseline);
    IdentityService::new(first.store.clone())
        .revoke_agent(server.credentials.agent_id, &first.lease)
        .await
        .unwrap();
    assert!(legitimate.validate_profile(&mail()).await.is_err());
    assert!(
        AgentClient::new(&first.store, server.credentials.clone(), &first.lease)
            .await
            .is_err()
    );
    assert_eq!(server.received.load(Ordering::SeqCst), baseline);
}
#[tokio::test]
async fn incompatible_agent_receives_neither_validation_profile_nor_job_command() {
    let center = fixtures::center().await;
    let (agent, control) = fixtures::job_agent::start(&center).await;
    agent.protocol.store(2, Ordering::SeqCst);
    let client = AgentClient::new(&center.store, agent.credentials.clone(), &center.lease)
        .await
        .unwrap();
    assert!(matches!(
        client.validate_profile(&mail()).await,
        Err(hmail_cleaner_center::agents::AgentError::Protocol)
    ));
    assert_eq!(agent.received.load(Ordering::SeqCst), 0);
    let command = hmail_cleaner_protocol::JobCommand {
        protocol_version: 1,
        job_id: hmail_cleaner_protocol::JobId::new(),
        agent_id: agent.credentials.agent_id,
        profile_id: hmail_cleaner_protocol::ProfileId::new(),
        profile_version: 1.into(),
        operation: hmail_cleaner_protocol::Operation::Preview,
        scan: hmail_cleaner_protocol::Scan::Fast,
        resolved_cutoff: fixtures::jobs::cutoff(),
        service_policy: hmail_cleaner_protocol::ServicePolicy::RequireStopped,
        limits: hmail_cleaner_protocol::RunLimits::default(),
        mail_profile: mail(),
    };
    assert!(matches!(
        client.submit_job(&command).await,
        Err(hmail_cleaner_center::agents::AgentError::Protocol)
    ));
    assert_eq!(control.puts(), 0);
}
#[tokio::test]
async fn revoked_agent_trust_does_not_report_that_the_admin_session_ended() {
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
    let (agent, _) = fixtures::job_agent::start(&center).await;
    let target = fixtures::jobs::configured(
        &center,
        agent.credentials.agent_id,
        agent.credentials.endpoint.as_str(),
    )
    .await;
    IdentityService::new(center.store.clone())
        .revoke_agent(agent.credentials.agent_id, &center.lease)
        .await
        .unwrap();
    let state = AppState::new(
        center.store.clone(),
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap()
    .with_authority(center.authority.clone());
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/servers/{}/validate", target.server_id))
        .header("host", "localhost:8443")
        .header("origin", "https://localhost:8443")
        .header("x-hmail-request", "1")
        .header("content-type", "application/json")
        .header(
            "cookie",
            format!("__Host-hmail-session={}", center.token.as_str()),
        )
        .body(Body::from("{}"))
        .unwrap();
    let response = router(state).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(center.authority.authenticate(&center.token).is_ok());
}
