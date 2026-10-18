#![cfg(unix)]
use astra_updater::journal::{load, save, OperationLock, Phase, Transaction};
use uuid::Uuid;
#[test]
fn atomic_journal_and_single_process_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    assert!(load::<serde_json::Value>(&path).unwrap().is_none());
    save(&path, &serde_json::json!({"phase":"staged"})).unwrap();
    assert_eq!(
        load::<serde_json::Value>(&path).unwrap().unwrap()["phase"],
        "staged"
    );
    let _lock = OperationLock::acquire(&dir.path().join("lock")).unwrap();
    assert!(OperationLock::acquire(&dir.path().join("lock")).is_err());
    std::fs::write(&path, b"broken").unwrap();
    assert!(load::<serde_json::Value>(&path).is_err());
}
#[test]
fn rollback_is_forbidden_as_soon_as_exposure_may_have_happened() {
    let mut tx = Transaction {
        version: 1,
        id: Uuid::new_v4(),
        phase: Phase::Verified,
        previous_image: String::new(),
        previous_build: String::new(),
        candidate_image: String::new(),
        candidate_build: String::new(),
        sequence: 1,
        backup: "/tmp/backup".into(),
        candidate_export: "/tmp/export".into(),
        previous_unit: "/tmp/unit".into(),
    };
    assert!(tx.can_restore());
    tx.phase = Phase::Exposing;
    assert!(!tx.can_restore());
    tx.phase = Phase::Committed;
    assert!(!tx.can_restore());
}
#[test]
fn dangling_state_symlink_is_corruption_not_absence() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.json");
    std::os::unix::fs::symlink(directory.path().join("missing"), &path).unwrap();
    assert!(load::<serde_json::Value>(&path).is_err());
}
