use hmail_cleaner_center::{
    account::{initialize, InitInput, SecretPrompt},
    storage::{CenterStore, DataDirectory},
    vault::{open_password, open_recovery, RecoveryKey, SecretText},
};
struct Prompt {
    recovery: Option<RecoveryKey>,
}
impl SecretPrompt for Prompt {
    fn password(&mut self) -> Result<SecretText, String> {
        Ok(SecretText::new("a-portable-test-password".into()))
    }
    fn confirm_recovery(&mut self, key: &SecretText) -> Result<bool, String> {
        self.recovery = Some(RecoveryKey::parse(key).unwrap());
        Ok(true)
    }
}
#[tokio::test]
async fn snapshot_restores_both_unlock_methods_without_plaintext() {
    let source = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(source.path()).unwrap();
    let store = CenterStore::open(&lease).await.unwrap();
    let mut prompt = Prompt { recovery: None };
    let receipt = initialize(
        &store,
        &InitInput {
            login: "admin".into(),
        },
        &mut prompt,
    )
    .await
    .unwrap();
    let account = store.account().await.unwrap().unwrap();
    let password = SecretText::new("a-portable-test-password".into());
    let dek = open_password(&account.material, &password).unwrap();
    let record_id = uuid::Uuid::new_v4();
    let header = hmail_cleaner_center::vault::EnvelopeHeader::record(
        hmail_cleaner_center::vault::RecordIdentity {
            vault_id: account.material.vault_id(),
            kind: hmail_cleaner_center::vault::RecordKind::Profile,
            record_id,
            record_version: 1,
            key_id: account.material.dek_id(),
            key_generation: 1,
        },
    )
    .unwrap();
    let sentinels = [
        "mysql-host-private",
        "mysql-port-private",
        "mysql-database-private",
        "mysql-user-private",
        "mysql-password-private",
        "D:\\mail-data-private",
        "C:\\hmail-ini-private",
        "mail-service-private",
        "agent-private-key-private",
    ];
    let plaintext = zeroize::Zeroizing::new(serde_json::to_vec(&sentinels).unwrap());
    let record = hmail_cleaner_center::vault::seal_record(&dek, &header, &plaintext).unwrap();
    store
        .insert_encrypted(record_id, "profile", 1, &record)
        .await
        .unwrap();
    let target = tempfile::tempdir().unwrap();
    let destination = target.path().join("backup");
    let manifest = store.backup(&destination).await.unwrap();
    assert_eq!(manifest.vault_id, Some(receipt.vault_id));
    assert!(store.backup(&destination).await.is_err());
    let restored_lease = DataDirectory::acquire(&destination).unwrap();
    let restored = CenterStore::open(&restored_lease).await.unwrap();
    let account = restored.account().await.unwrap().unwrap();
    let record = restored
        .encrypted(record_id, &header)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        *hmail_cleaner_center::vault::open_record(&dek, &header, &record).unwrap(),
        *plaintext
    );
    let password = SecretText::new("a-portable-test-password".into());
    assert_eq!(
        *open_password(&account.material, &password).unwrap(),
        *open_recovery(&account.material, &prompt.recovery.unwrap()).unwrap()
    );
    for root in [source.path(), destination.as_path()] {
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                let bytes = std::fs::read(path).unwrap();
                assert!(!bytes
                    .windows(password.len())
                    .any(|w| w == password.as_bytes()));
                for sentinel in sentinels {
                    assert!(!bytes
                        .windows(sentinel.len())
                        .any(|w| w == sentinel.as_bytes()));
                }
            }
        }
    }
}
