pub mod fixtures;
use hmail_cleaner_center::{
    account::AccountService,
    vault::{self, RecoveryKey, SecretText},
};
use hmail_cleaner_protocol::browser::auth::ChangePasswordInput;
#[tokio::test]
async fn password_change_rewraps_only_and_recovery_stays_valid() {
    let center = fixtures::center().await;
    fixtures::jobs::register(&center, "https://127.0.0.1:9444").await;
    let before = center.store.account().await.unwrap().unwrap();
    let before_json = serde_json::to_value(&before.material).unwrap();
    use sqlx::{sqlite::SqliteConnectOptions, Connection};
    let mut database = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(center.root.path().join("center.sqlite")),
    )
    .await
    .unwrap();
    let ciphertexts: Vec<Vec<u8>> =
        sqlx::query_scalar("SELECT envelope FROM encrypted_records ORDER BY id")
            .fetch_all(&mut database)
            .await
            .unwrap();
    let recovery = RecoveryKey::parse(&center.recovery).unwrap();
    let service = AccountService::new(center.store.clone(), center.authority.clone());
    service
        .change_password(
            &center.lease,
            ChangePasswordInput {
                current_password: SecretText::new("fixture-center-password".into()),
                new_password: SecretText::new("fresh-center-password-2026".into()),
                confirm_password: SecretText::new("fresh-center-password-2026".into()),
            },
        )
        .await
        .unwrap();
    let after = center.store.account().await.unwrap().unwrap();
    let after_json = serde_json::to_value(&after.material).unwrap();
    assert_eq!(before_json["recovery_wrap"], after_json["recovery_wrap"]);
    assert_ne!(before_json["password_wrap"], after_json["password_wrap"]);
    let unchanged: Vec<Vec<u8>> =
        sqlx::query_scalar("SELECT envelope FROM encrypted_records ORDER BY id")
            .fetch_all(&mut database)
            .await
            .unwrap();
    assert_eq!(ciphertexts, unchanged);
    assert_eq!(after.revision, before.revision + 1);
    assert!(vault::open_password(
        &after.material,
        &SecretText::new("fixture-center-password".into())
    )
    .is_err());
    assert!(vault::open_password(
        &after.material,
        &SecretText::new("fresh-center-password-2026".into())
    )
    .is_ok());
    assert!(vault::open_recovery(&after.material, &recovery).is_ok());
    assert_eq!(after.material.dek_id(), before.material.dek_id());
    assert!(center.authority.authenticate(&center.token).is_err());
    let backup = center.root.path().join("credential-backup");
    center.store.backup(&backup).await.unwrap();
    for directory in [center.root.path(), backup.as_path()] {
        for item in std::fs::read_dir(directory).unwrap() {
            let path = item.unwrap().path();
            if !path.is_file() {
                continue;
            }
            let bytes = std::fs::read(path).unwrap();
            for secret in [
                "fixture-center-password",
                "fresh-center-password-2026",
                "NEVER-PERSIST-JOB-PASSWORD",
                center.recovery.as_str(),
            ] {
                assert!(!bytes
                    .windows(secret.len())
                    .any(|value| value == secret.as_bytes()));
            }
        }
    }
}
