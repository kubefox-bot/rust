#![cfg(unix)]
use hmail_cleaner_center::{
    account::{initialize, InitInput, SecretPrompt},
    config::CenterConfig,
    storage::{CenterStore, DataDirectory},
    vault::SecretText,
};
struct Prompt;
impl SecretPrompt for Prompt {
    fn password(&mut self) -> Result<SecretText, String> {
        Ok(SecretText::new("fixture-appliance-password".into()))
    }
    fn confirm_recovery(&mut self, _: &SecretText) -> Result<bool, String> {
        Ok(true)
    }
}
#[tokio::test]
async fn updater_accepts_real_current_backup_and_rejects_schema_mismatch() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    CenterConfig::create(
        root.path(),
        "https://localhost:8443",
        "0.0.0.0:8443".parse().unwrap(),
    )
    .unwrap();
    let store = CenterStore::open(&lease).await.unwrap();
    initialize(
        &store,
        &InitInput {
            login: "admin".into(),
        },
        &mut Prompt,
    )
    .await
    .unwrap();
    let parent = tempfile::tempdir().unwrap();
    let snapshot = parent.path().join("snapshot");
    let manifest = store
        .backup_with_build(&snapshot, "fixture-appliance")
        .await
        .unwrap();
    assert_eq!(manifest.schema_version, 2);
    assert!(
        astra_updater::backup::verify(&snapshot, "fixture-appliance").is_ok(),
        "the updater must accept the actual center backup"
    );
    let path = snapshot.join("manifest.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for schema in [1, 3] {
        value["schema_version"] = serde_json::json!(schema);
        astra_updater::journal::save(&path, &value).unwrap();
        assert!(
            astra_updater::backup::verify(&snapshot, "fixture-appliance").is_err(),
            "manifest schema {schema} must not describe the schema 2 SQLite file"
        );
    }
}
