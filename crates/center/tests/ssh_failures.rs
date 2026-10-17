pub mod fixtures;
use fixtures::enrollment::{self, request, settled};
use hmail_cleaner_center::enrollment::EnrollmentService;
use hmail_cleaner_protocol::browser::enrollment::EnrollmentPhase;
use std::sync::atomic::Ordering;
#[tokio::test]
async fn partial_upload_never_installs_and_secrets_are_not_checkpoints() {
    let center = fixtures::center().await;
    let packages = enrollment::packages();
    let node = enrollment::connector();
    node.0.fail_upload.store(true, Ordering::SeqCst);
    let service =
        EnrollmentService::new(center.store.clone(), packages.catalog.clone(), node.clone());
    let input = request(&node);
    let id = input.operation_id;
    service.install(&center.lease, input).await.unwrap();
    let failed = settled(&service, id, &center.lease).await;
    assert_eq!(failed.phase, EnrollmentPhase::Failed);
    assert!(!failed.slot_held);
    assert_eq!(node.0.installs.load(Ordering::SeqCst), 0);
    assert_eq!(node.0.closes.load(Ordering::SeqCst), 1);
    for name in ["center.sqlite", "center.sqlite-wal"] {
        if let Ok(bytes) = std::fs::read(center.root.path().join(name)) {
            assert!(!bytes
                .windows(b"OS-PASSWORD-NEVER-PERSIST".len())
                .any(|part| part == b"OS-PASSWORD-NEVER-PERSIST"));
        }
    }
}
#[tokio::test]
async fn lost_final_install_response_reconciles_exact_operation_without_reinstall() {
    let center = fixtures::center().await;
    let packages = enrollment::packages();
    let node = enrollment::connector();
    node.0.lost_install_response.store(true, Ordering::SeqCst);
    let service = EnrollmentService::new(center.store.clone(), packages.catalog, node.clone());
    let input = request(&node);
    let id = input.operation_id;
    service.install(&center.lease, input).await.unwrap();
    let unknown = settled(&service, id, &center.lease).await;
    assert_eq!(unknown.phase, EnrollmentPhase::Unknown);
    assert!(unknown.slot_held);
    service.recover_interrupted().await.unwrap();
    let path = node.0.data.join("installation.json");
    let original = std::fs::read(&path).unwrap();
    let mut wrong: serde_json::Value = serde_json::from_slice(&original).unwrap();
    wrong["operation_id"] = serde_json::json!(uuid::Uuid::new_v4());
    std::fs::write(&path, serde_json::to_vec(&wrong).unwrap()).unwrap();
    let reconciling = service.clone();
    let lease = center.lease.clone();
    let task = tokio::spawn(async move { reconciling.reconcile(id, &lease).await });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(service.get(id, &center.lease).await.unwrap().slot_held);
    assert!(!task.is_finished());
    std::fs::write(&path, original).unwrap();
    let ready = task.await.unwrap().unwrap();
    assert_eq!(ready.phase, EnrollmentPhase::Ready);
    assert!(!ready.slot_held);
    assert_eq!(node.0.installs.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn wrong_package_checksum_is_rejected_before_any_ssh_connection() {
    let center = fixtures::center().await;
    let packages = enrollment::packages();
    let node = enrollment::connector();
    std::fs::write(
        packages
            .root
            .path()
            .join(enrollment::BUILD)
            .join("hmail-cleaner-agent.exe"),
        b"MZ bad file",
    )
    .unwrap();
    let service = EnrollmentService::new(center.store.clone(), packages.catalog, node.clone());
    assert!(service
        .install(&center.lease, request(&node))
        .await
        .is_err());
    assert_eq!(node.0.connects.load(Ordering::SeqCst), 0);
}
