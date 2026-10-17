use super::IdentityManager;
#[path = "../../tests/fixtures/bootstrap.rs"]
pub mod fixture;
use crate::{config::AgentConfig, jobs::JobRepository};
use hmail_cleaner_protocol::{AgentIdentityInstall, IdentityReceipt, PROTOCOL_VERSION};
use std::sync::Arc;
fn candidate(
    manager: &IdentityManager,
    ca: &rcgen::CertifiedIssuer<'_, rcgen::KeyPair>,
    agent: hmail_cleaner_protocol::AgentId,
) -> AgentIdentityInstall {
    let csr = manager.csr().unwrap();
    assert!(!csr.csr_pem.contains("PRIVATE KEY"));
    let mut request = rcgen::CertificateSigningRequestParams::from_pem(&csr.csr_pem).unwrap();
    let mut params = rcgen::CertificateParams::new(vec!["agent.internal".into()]).unwrap();
    params.subject_alt_names.push(rcgen::SanType::URI(
        format!("urn:hmail-cleaner:agent:{agent}")
            .try_into()
            .unwrap(),
    ));
    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    params.serial_number = Some(91u64.into());
    request.params = params;
    AgentIdentityInstall {
        protocol_version: PROTOCOL_VERSION,
        agent_id: agent,
        server_name: "agent.internal".into(),
        certificate_pem: request.signed_by(ca).unwrap().pem(),
    }
}
#[tokio::test]
async fn renewal_keeps_private_key_and_changes_tls_only_after_durable_install() {
    let root = tempfile::tempdir().unwrap();
    let (path, agent, ca) = fixture::signed(root.path());
    let config = AgentConfig::load(&path, "fixture").unwrap();
    let repository = Arc::new(
        JobRepository::open(&config.data_directory.join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let manager = IdentityManager::new(&config, repository.clone());
    let key = std::fs::read(config.data_directory.join("identity-key.pem")).unwrap();
    let (_, prior) = config.tls.snapshot();
    let request = candidate(&manager, &ca, agent);
    let repeat = request.clone();
    let expected = request.certificate_pem.clone();
    let result: IdentityReceipt = manager
        .install(request, || panic!("successful renewal must remain ready"))
        .await
        .unwrap();
    assert_eq!(result.agent_id, agent);
    assert!(std::fs::read(config.data_directory.join("identity-key.pem")).unwrap() == key);
    assert_eq!(
        std::fs::read_to_string(config.data_directory.join("identity.pem")).unwrap(),
        expected
    );
    assert!(!repository.in_maintenance().await.unwrap());
    tokio::time::timeout(std::time::Duration::from_secs(1), prior.cancelled())
        .await
        .unwrap();
    manager
        .install(repeat, || panic!("idempotent renewal must remain ready"))
        .await
        .unwrap();
}
#[tokio::test]
async fn wrong_identity_and_active_receipt_preserve_existing_certificate() {
    let root = tempfile::tempdir().unwrap();
    let (path, agent, ca) = fixture::signed(root.path());
    let config = AgentConfig::load(&path, "fixture").unwrap();
    let certificate = std::fs::read(config.data_directory.join("identity.pem")).unwrap();
    let repository = Arc::new(
        JobRepository::open(&config.data_directory.join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let manager = IdentityManager::new(&config, repository.clone());
    let mut wrong = candidate(&manager, &ca, hmail_cleaner_protocol::AgentId::new());
    wrong.agent_id = agent;
    assert!(manager
        .install(wrong, || panic!("invalid input must not change readiness"))
        .await
        .is_err());
    let mut command: hmail_cleaner_protocol::JobCommand =
        serde_json::from_str(include_str!("../../../protocol/tests/fixtures/job-v1.json")).unwrap();
    command.agent_id = agent;
    let job = hmail_cleaner_protocol::validate_job(command).unwrap();
    repository
        .accept(&job, repository.fingerprint(&job))
        .await
        .unwrap();
    assert!(matches!(
        manager
            .install(candidate(&manager, &ca, agent), || panic!(
                "busy must not change readiness"
            ))
            .await,
        Err(crate::execution::AgentError::Busy)
    ));
    assert!(std::fs::read(config.data_directory.join("identity.pem")).unwrap() == certificate);
}
#[tokio::test]
async fn incomplete_certificate_write_freezes_admission_and_signals_unhealthy() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let root = tempfile::tempdir().unwrap();
    let (path, agent, ca) = fixture::signed(root.path());
    let config = AgentConfig::load(&path, "fixture").unwrap();
    let original = std::fs::read(config.data_directory.join("identity.pem")).unwrap();
    let repository = Arc::new(
        JobRepository::open(&config.data_directory.join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let manager = IdentityManager::new(&config, repository.clone());
    std::fs::write(
        config.data_directory.join("identity.pem.new"),
        b"incomplete stage",
    )
    .unwrap();
    let failed = Arc::new(AtomicBool::new(false));
    let signal = failed.clone();
    assert!(manager
        .install(candidate(&manager, &ca, agent), move || {
            signal.store(true, Ordering::SeqCst);
        })
        .await
        .is_err());
    assert!(failed.load(Ordering::SeqCst));
    assert!(repository.in_maintenance().await.unwrap());
    assert!(std::fs::read(config.data_directory.join("identity.pem")).unwrap() == original);
}
#[tokio::test]
async fn a_disconnected_waiter_does_not_abandon_an_admitted_certificate_update() {
    use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection};
    let root = tempfile::tempdir().unwrap();
    let (path, agent, ca) = fixture::signed(root.path());
    let config = AgentConfig::load(&path, "fixture").unwrap();
    let database = config.data_directory.join("jobs.sqlite");
    let repository = Arc::new(JobRepository::open(&database).await.unwrap());
    let manager = IdentityManager::new(&config, repository.clone());
    let input = candidate(&manager, &ca, agent);
    let expected = input.certificate_pem.clone();
    let mut blocker =
        SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(database))
            .await
            .unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut blocker)
        .await
        .unwrap();
    let waiter = tokio::spawn(async move {
        manager
            .install(input, || panic!("admitted update should complete"))
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match crate::service::acquire_update_lock(&config.data_directory) {
                Err(crate::service::InstallError::Busy) => break,
                Ok(lock) => drop(lock),
                Err(_) => panic!("fixture update lock unavailable"),
            };
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    waiter.abort();
    assert!(matches!(waiter.await,Err(error) if error.is_cancelled()));
    sqlx::query("COMMIT").execute(&mut blocker).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if std::fs::read_to_string(config.data_directory.join("identity.pem")).unwrap()
                == expected
                && !repository.in_maintenance().await.unwrap()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
