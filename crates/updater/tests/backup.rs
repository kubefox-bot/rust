#![cfg(unix)]
use astra_updater::{
    backup,
    journal::{self, Phase, Transaction},
    verification::hash_file,
};
use uuid::Uuid;
#[test]
fn snapshot_receipt_must_cover_every_required_file() {
    let dir = tempfile::tempdir().unwrap();
    let snapshot = dir.path().join("snapshot");
    std::fs::create_dir(&snapshot).unwrap();
    for name in [
        "center.sqlite",
        "https-cert.pem",
        "https-key.pem",
        "ca-cert.pem",
        "center.json",
        "update-policy.json",
    ] {
        std::fs::write(snapshot.join(name), b"example").unwrap();
    }
    journal::save(&snapshot.join("manifest.json"),&serde_json::json!({"format_version":1,"schema_version":1,"vault_id":Uuid::new_v4(),"build_version":"old"})).unwrap();
    let tx = Transaction {
        version: 1,
        id: Uuid::new_v4(),
        phase: Phase::Starting,
        previous_image: "old".into(),
        previous_build: "old".into(),
        candidate_image: "next".into(),
        candidate_build: "next".into(),
        sequence: 2,
        backup: snapshot.clone(),
        candidate_export: dir.path().join("candidate.tar"),
        previous_unit: dir.path().join("old.container"),
    };
    let mut hashes = std::collections::BTreeMap::new();
    for name in [
        "https-cert.pem",
        "https-key.pem",
        "ca-cert.pem",
        "center.json",
        "manifest.json",
        "update-policy.json",
    ] {
        hashes.insert(name.to_owned(), hash_file(&snapshot.join(name)).unwrap());
    }
    journal::save(&dir.path().join("snapshot-hashes.json"), &hashes).unwrap();
    assert!(
        backup::verify_hashes(&tx).is_err(),
        "a same-sized receipt cannot substitute optional policy for unverified SQLite"
    );
    hashes.insert(
        "center.sqlite".into(),
        hash_file(&snapshot.join("center.sqlite")).unwrap(),
    );
    journal::save(&dir.path().join("snapshot-hashes.json"), &hashes).unwrap();
    assert!(backup::verify_hashes(&tx).is_ok());
    std::fs::write(snapshot.join("center.sqlite"), b"changed").unwrap();
    assert!(backup::verify_hashes(&tx).is_err());
}
