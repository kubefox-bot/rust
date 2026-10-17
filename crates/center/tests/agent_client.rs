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
