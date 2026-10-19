pub mod fixtures;
use hmail_cleaner_center::{agents::AgentClient, identity::IdentityService};
use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection};
#[tokio::test]
async fn pending_candidate_recovers_after_installed_tls_but_failed_center_commit() {
    let fixture = fixtures::center().await;
    let agent = fixtures::agents::native(&fixture).await;
    let previous = AgentClient::new(&fixture.store, agent.credentials.clone(), &fixture.lease)
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if previous.health().await.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let private = std::fs::read(agent.data_directory.join("identity-key.pem")).unwrap();
    let mut database = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(fixture.root.path().join("center.sqlite")),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER fail_activation BEFORE UPDATE ON trust BEGIN SELECT RAISE(ABORT,'fixture activation failure'); END").execute(&mut database).await.unwrap();
    let service = IdentityService::new(fixture.store.clone());
    assert!(service
        .renew_agent(agent.credentials.clone(), &fixture.lease)
        .await
        .is_err());
    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM identity_renewals")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(pending, 1);
    let installed = std::fs::read(agent.data_directory.join("identity.pem")).unwrap();
    sqlx::query("DROP TRIGGER fail_activation")
        .execute(&mut database)
        .await
        .unwrap();
    service
        .renew_agent(agent.credentials.clone(), &fixture.lease)
        .await
        .unwrap();
    assert!(std::fs::read(agent.data_directory.join("identity.pem")).unwrap() == installed);
    assert!(std::fs::read(agent.data_directory.join("identity-key.pem")).unwrap() == private);
    assert!(previous.health().await.is_err());
    assert!(
        AgentClient::new(&fixture.store, agent.credentials.clone(), &fixture.lease)
            .await
            .unwrap()
            .health()
            .await
            .is_ok()
    );
    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM identity_renewals")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(pending, 0);
}
#[tokio::test]
async fn an_active_agent_receipt_blocks_rotation_without_revoking_the_old_certificate() {
    let fixture = fixtures::center().await;
    let agent = fixtures::agents::native(&fixture).await;
    let old = std::fs::read(agent.data_directory.join("identity.pem")).unwrap();
    let repository =
        hmail_cleaner_agent::jobs::JobRepository::open(&agent.data_directory.join("jobs.sqlite"))
            .await
            .unwrap();
    let mut command: hmail_cleaner_protocol::JobCommand =
        serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap();
    command.agent_id = agent.credentials.agent_id;
    let job = hmail_cleaner_protocol::validate_job(command).unwrap();
    repository
        .accept(&job, repository.fingerprint(&job))
        .await
        .unwrap();
    let service = IdentityService::new(fixture.store.clone());
    assert!(service
        .renew_agent(agent.credentials.clone(), &fixture.lease)
        .await
        .is_err());
    assert!(std::fs::read(agent.data_directory.join("identity.pem")).unwrap() == old);
    assert!(
        AgentClient::new(&fixture.store, agent.credentials.clone(), &fixture.lease)
            .await
            .unwrap()
            .health()
            .await
            .is_ok()
    );
    repository.cancel(job.command().job_id).await.unwrap();
    service
        .renew_agent(agent.credentials.clone(), &fixture.lease)
        .await
        .unwrap();
    assert!(std::fs::read(agent.data_directory.join("identity.pem")).unwrap() != old);
}
#[tokio::test]
async fn revocation_cannot_be_undone_by_a_pending_renewal() {
    let fixture = fixtures::center().await;
    let agent = fixtures::agents::native(&fixture).await;
    let mut database = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(fixture.root.path().join("center.sqlite")),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER fail_activation BEFORE UPDATE ON trust BEGIN SELECT RAISE(ABORT,'fixture'); END").execute(&mut database).await.unwrap();
    let service = IdentityService::new(fixture.store.clone());
    assert!(service
        .renew_agent(agent.credentials.clone(), &fixture.lease)
        .await
        .is_err());
    sqlx::query("DROP TRIGGER fail_activation")
        .execute(&mut database)
        .await
        .unwrap();
    service
        .revoke_agent(agent.credentials.agent_id, &fixture.lease)
        .await
        .unwrap();
    assert!(service
        .renew_agent(agent.credentials.clone(), &fixture.lease)
        .await
        .is_err());
    let revoked: bool = sqlx::query_scalar("SELECT revoked FROM trust WHERE agent_id=?")
        .bind(agent.credentials.agent_id.to_string())
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert!(revoked);
}
#[derive(serde::Serialize, serde::Deserialize)]
struct ClientPayload {
    certificate_pem: String,
    private_key_pem: zeroize::Zeroizing<String>,
    ca_pem: String,
}
#[tokio::test]
async fn authenticated_renewal_refreshes_an_expired_center_client_before_agent_csr() {
    use hmail_cleaner_center::vault::{self, EnvelopeHeader, RecordIdentity, RecordKind};
    let fixture = fixtures::center().await;
    let agent = fixtures::agents::native(&fixture).await;
    let account = fixture.store.account().await.unwrap().unwrap();
    let header = |kind, record_id, record_version| {
        EnvelopeHeader::record(RecordIdentity {
            vault_id: account.material.vault_id(),
            kind,
            record_id,
            record_version,
            key_id: account.material.dek_id(),
            key_generation: 1,
        })
        .unwrap()
    };
    let ca = fixtures::identity::ca(&fixture).await;
    let issuer = rcgen::Issuer::from_ca_cert_pem(
        &ca.certificate_pem,
        rcgen::KeyPair::from_pem(&ca.private_key_pem).unwrap(),
    )
    .unwrap();
    let client_header = header(
        RecordKind::CenterClientKey,
        account.material.account_id(),
        1,
    );
    let client = fixture
        .store
        .encrypted(account.material.account_id(), &client_header)
        .await
        .unwrap()
        .unwrap();
    let plain = fixture
        .lease
        .with_dek(|dek| vault::open_record(dek, &client_header, &client))
        .unwrap()
        .unwrap();
    let mut payload: ClientPayload = serde_json::from_slice(&plain).unwrap();
    let private = payload.private_key_pem.clone();
    let key = rcgen::KeyPair::from_pem(&payload.private_key_pem).unwrap();
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    params.subject_alt_names.push(rcgen::SanType::URI(
        format!("urn:hmail-cleaner:center:{}", account.material.vault_id())
            .try_into()
            .unwrap(),
    ));
    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ClientAuth];
    params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
    params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(2);
    params.not_after = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    payload.certificate_pem = format!(
        "{}{}",
        params.signed_by(&key, &issuer).unwrap().pem(),
        ca.certificate_pem
    );
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&payload).unwrap());
    let new_header = header(
        RecordKind::CenterClientKey,
        account.material.account_id(),
        2,
    );
    let envelope = fixture
        .lease
        .with_dek(|dek| vault::seal_record(dek, &new_header, &bytes))
        .unwrap()
        .unwrap();
    let mut database = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(fixture.root.path().join("center.sqlite")),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE encrypted_records SET version=2,envelope=? WHERE id=?")
        .bind(envelope.encode().unwrap())
        .bind(account.material.account_id().to_string())
        .execute(&mut database)
        .await
        .unwrap();
    assert!(
        AgentClient::new(&fixture.store, agent.credentials.clone(), &fixture.lease)
            .await
            .unwrap()
            .health()
            .await
            .is_err()
    );
    IdentityService::new(fixture.store.clone())
        .renew_agent(agent.credentials.clone(), &fixture.lease)
        .await
        .unwrap();
    let next_header = header(
        RecordKind::CenterClientKey,
        account.material.account_id(),
        3,
    );
    let encrypted = fixture
        .store
        .encrypted(account.material.account_id(), &next_header)
        .await
        .unwrap()
        .unwrap();
    let plain = fixture
        .lease
        .with_dek(|dek| vault::open_record(dek, &next_header, &encrypted))
        .unwrap()
        .unwrap();
    let updated: ClientPayload = serde_json::from_slice(&plain).unwrap();
    assert!(updated.private_key_pem == private);
    assert!(
        AgentClient::new(&fixture.store, agent.credentials.clone(), &fixture.lease)
            .await
            .unwrap()
            .health()
            .await
            .is_ok()
    );
}
