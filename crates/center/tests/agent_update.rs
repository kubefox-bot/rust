pub mod fixtures;
use fixtures::enrollment::{self, request, settled};
use hmail_cleaner_center::enrollment::{EnrollmentError, EnrollmentService};
use hmail_cleaner_protocol::browser::enrollment::{
    EnrollmentPhase, SshCredentials, SshTarget, UpdateRequest,
};
use std::sync::atomic::Ordering;
#[tokio::test]
async fn successful_install_is_unconfigured_and_update_preserves_local_key_and_certificate() {
    let center = fixtures::center().await;
    let packages = enrollment::packages();
    let node = enrollment::connector();
    let service = EnrollmentService::new(center.store.clone(), packages.catalog, node.clone());
    let input = request(&node);
    let (id, profile) = (input.operation_id, input.profile_id);
    let key = std::fs::read(node.0.data.join("identity-key.pem")).unwrap();
    service.install(&center.lease, input).await.unwrap();
    let ready = settled(&service, id, &center.lease).await;
    assert_eq!(ready.phase, EnrollmentPhase::Ready);
    assert!(!ready.slot_held);
    let editable = hmail_cleaner_center::profiles::ProfileService::new(center.store.clone())
        .read(profile, &center.lease)
        .await
        .unwrap();
    assert!(!editable.configured);
    let certificate = std::fs::read(node.0.data.join("identity.pem")).unwrap();
    let input = request(&node);
    let update = UpdateRequest {
        mode: Default::default(),
        operation_id: input.operation_id,
        target: input.target,
        fingerprint: input.fingerprint,
        credentials: input.credentials,
        package_version: input.package_version,
    };
    let update_id = update.operation_id;
    service
        .update(&center.lease, node.0.id, update)
        .await
        .unwrap();
    let updated = settled(&service, update_id, &center.lease).await;
    assert_eq!(updated.phase, EnrollmentPhase::Ready);
    assert_eq!(
        std::fs::read(node.0.data.join("identity-key.pem")).unwrap(),
        key
    );
    assert_eq!(
        std::fs::read(node.0.data.join("identity.pem")).unwrap(),
        certificate
    );
    assert_eq!(node.0.installs.load(Ordering::SeqCst), 2);
    let repo = hmail_cleaner_agent::jobs::JobRepository::open(&node.0.data.join("jobs.sqlite"))
        .await
        .unwrap();
    assert!(repo.begin_maintenance().await.unwrap());
    repo.end_maintenance().await.unwrap();
    repo.close().await;
}
#[tokio::test]
async fn busy_agent_refuses_update_before_ssh_or_upload() {
    let center = fixtures::center().await;
    let packages = enrollment::packages();
    let node = enrollment::connector();
    let agent = fixtures::agents::start(&center, hmail_cleaner_protocol::AgentId::new()).await;
    fixtures::jobs::configured(
        &center,
        agent.credentials.agent_id,
        agent.credentials.endpoint.as_str(),
    )
    .await;
    agent.idle.store(false, Ordering::SeqCst);
    let service = EnrollmentService::new(center.store.clone(), packages.catalog, node.clone());
    let input = UpdateRequest {
        mode: Default::default(),
        operation_id: uuid::Uuid::new_v4(),
        target: SshTarget {
            host: "127.0.0.1".into(),
            port: 22,
        },
        fingerprint: "SHA256:".to_owned() + &"A".repeat(43),
        credentials: SshCredentials::Password {
            username: "admin".into(),
            password: zeroize::Zeroizing::new("fixture".into()),
        },
        package_version: enrollment::BUILD.into(),
    };
    assert!(matches!(
        service
            .update(&center.lease, agent.credentials.agent_id, input)
            .await,
        Err(EnrollmentError::Busy)
    ));
    assert_eq!(node.0.connects.load(Ordering::SeqCst), 0);
    assert_eq!(node.0.installs.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn logout_stops_new_privileged_steps_and_releases_a_preinstall_reservation() {
    let center = fixtures::center().await;
    let packages = enrollment::packages();
    let node = enrollment::connector();
    *node.0.revoke_on_prepare.lock().unwrap() =
        Some((center.authority.clone(), center.lease.generation()));
    let service = EnrollmentService::new(center.store.clone(), packages.catalog, node.clone());
    let input = request(&node);
    let id = input.operation_id;
    service.install(&center.lease, input).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while node.0.closes.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(node.0.uploads.load(Ordering::SeqCst), 0);
    assert_eq!(node.0.installs.load(Ordering::SeqCst), 0);
    assert!(service.get(id, &center.lease).await.is_err());
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        center.root.path().join("center.sqlite").display()
    ))
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let phase: String = sqlx::query_scalar("SELECT phase FROM enrollment WHERE id=?")
                .bind(id.to_string())
                .fetch_one(&pool)
                .await
                .unwrap();
            if phase == "failed" {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let slot: Option<String> =
        sqlx::query_scalar("SELECT enrollment_id FROM provisioning_slot WHERE singleton=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(slot.is_none());
    pool.close().await;
}
#[tokio::test]
async fn verified_ssh_can_restore_expired_agent_trust_without_replacing_its_private_key() {
    use hmail_cleaner_protocol::browser::enrollment::UpdateMode;
    let center = fixtures::center().await;
    let packages = enrollment::packages();
    let node = enrollment::connector();
    let service = EnrollmentService::new(center.store.clone(), packages.catalog, node.clone());
    let initial = request(&node);
    let id = initial.operation_id;
    service.install(&center.lease, initial).await.unwrap();
    assert_eq!(
        settled(&service, id, &center.lease).await.phase,
        EnrollmentPhase::Ready
    );
    let key = std::fs::read(node.0.data.join("identity-key.pem")).unwrap();
    node.stop().await;
    fixtures::identity::expire_agent(&center, &node.0.data, node.0.id).await;
    node.restart().await;
    let credentials = hmail_cleaner_center::profiles::ProfileService::new(center.store.clone())
        .credentials_for_server(
            service.get(id, &center.lease).await.unwrap().server_id,
            &center.lease,
        )
        .await
        .unwrap();
    assert!(hmail_cleaner_center::agents::AgentClient::new(
        &center.store,
        credentials,
        &center.lease
    )
    .await
    .unwrap()
    .health()
    .await
    .is_err());
    let input = request(&node);
    let id = input.operation_id;
    service
        .update(
            &center.lease,
            node.0.id,
            UpdateRequest {
                mode: UpdateMode::RecoverIdentity,
                operation_id: id,
                target: input.target,
                fingerprint: input.fingerprint,
                credentials: input.credentials,
                package_version: input.package_version,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        settled(&service, id, &center.lease).await.phase,
        EnrollmentPhase::Ready
    );
    assert_eq!(
        std::fs::read(node.0.data.join("identity-key.pem")).unwrap(),
        key
    );
}
#[tokio::test]
async fn revocation_during_ssh_recovery_cannot_reactivate_trust() {
    use hmail_cleaner_protocol::browser::enrollment::UpdateMode;
    let center = fixtures::center().await;
    let packages = enrollment::packages();
    let node = enrollment::connector();
    let service = EnrollmentService::new(center.store.clone(), packages.catalog, node.clone());
    let input = request(&node);
    let id = input.operation_id;
    service.install(&center.lease, input).await.unwrap();
    assert_eq!(
        settled(&service, id, &center.lease).await.phase,
        EnrollmentPhase::Ready
    );
    *node.0.revoke_on_csr.lock().unwrap() = Some((center.store.clone(), center.lease.clone()));
    let input = request(&node);
    let id = input.operation_id;
    service
        .update(
            &center.lease,
            node.0.id,
            UpdateRequest {
                mode: UpdateMode::RecoverIdentity,
                operation_id: id,
                target: input.target,
                fingerprint: input.fingerprint,
                credentials: input.credentials,
                package_version: input.package_version,
            },
        )
        .await
        .unwrap();
    let view = settled(&service, id, &center.lease).await;
    assert_eq!(view.phase, EnrollmentPhase::Unknown);
    assert!(view.slot_held);
    assert_eq!(node.0.installs.load(Ordering::SeqCst), 1);
}
