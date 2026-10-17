pub mod fixtures;
use hmail_cleaner_center::{
    account::{self, AccountService, SecretPrompt},
    storage::DataDirectory,
    vault::{self, RecoveryKey, SecretText},
};
use hmail_cleaner_protocol::browser::auth::ChangePasswordInput;
fn change() -> ChangePasswordInput {
    ChangePasswordInput {
        current_password: SecretText::new("fixture-center-password".into()),
        new_password: SecretText::new("fresh-center-password-2026".into()),
        confirm_password: SecretText::new("fresh-center-password-2026".into()),
    }
}
#[tokio::test]
async fn stale_login_and_candidate_cannot_win_after_password_commit() {
    let f = fixtures::center().await;
    let before = f.store.account().await.unwrap().unwrap();
    let stale = vault::open_password(&before.material, &change().current_password).unwrap();
    let ticket = f.authority.begin_login().unwrap();
    let service = AccountService::new(f.store.clone(), f.authority.clone());
    let candidate = service
        .prepare_recovery(&f.lease, change().current_password)
        .await
        .unwrap();
    service.change_password(&f.lease, change()).await.unwrap();
    assert!(f
        .authority
        .admit_ticket(ticket, before.revision, stale)
        .is_err());
    assert!(service
        .confirm_recovery(
            &f.lease,
            candidate.candidate_id,
            &RecoveryKey::parse(&candidate.recovery_key).unwrap()
        )
        .await
        .is_err());
}
struct Prompt {
    recovery: Option<SecretText>,
    confirm: bool,
}
impl SecretPrompt for Prompt {
    fn password(&mut self) -> Result<SecretText, String> {
        Ok(change().new_password)
    }
    fn confirm_recovery(&mut self, key: &SecretText) -> Result<bool, String> {
        self.recovery = Some(key.clone());
        Ok(self.confirm)
    }
}
#[tokio::test]
async fn local_restore_replaces_both_wraps_after_confirmation_and_keeps_dek() {
    let f = fixtures::center().await;
    let before = f.store.account().await.unwrap().unwrap();
    let old = RecoveryKey::parse(&f.recovery).unwrap();
    let mut prompt = Prompt {
        recovery: None,
        confirm: false,
    };
    assert!(account::restore(&f.store, &old, &mut prompt).await.is_err());
    assert_eq!(f.store.account().await.unwrap().unwrap().revision, 1);
    prompt.confirm = true;
    account::restore(&f.store, &old, &mut prompt).await.unwrap();
    let after = f.store.account().await.unwrap().unwrap();
    assert_eq!(before.material.dek_id(), after.material.dek_id());
    assert!(vault::open_password(&after.material, &change().current_password).is_err());
    assert!(vault::open_recovery(&after.material, &old).is_err());
    assert!(vault::open_password(&after.material, &change().new_password).is_ok());
    assert!(vault::open_recovery(
        &after.material,
        &RecoveryKey::parse(&prompt.recovery.unwrap()).unwrap()
    )
    .is_ok());
}
#[tokio::test]
async fn running_center_refuses_local_recovery_lock_before_secrets() {
    let f = fixtures::center().await;
    assert!(DataDirectory::acquire(f.root.path()).is_err());
    assert_eq!(f.store.account().await.unwrap().unwrap().revision, 1);
}

async fn database(f: &fixtures::CenterFixture) -> sqlx::SqliteConnection {
    use sqlx::{sqlite::SqliteConnectOptions, Connection};
    sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(f.root.path().join("center.sqlite")),
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn rollback_preserves_both_unlock_methods() {
    let f = fixtures::center().await;
    let before = f
        .store
        .account()
        .await
        .unwrap()
        .unwrap()
        .material
        .encode()
        .unwrap();
    let mut db = database(&f).await;
    sqlx::query("CREATE TRIGGER reject_credentials BEFORE UPDATE ON account BEGIN SELECT RAISE(ABORT,'fixture failure'); END").execute(&mut db).await.unwrap();
    let service = AccountService::new(f.store.clone(), f.authority.clone());
    assert!(service.change_password(&f.lease, change()).await.is_err());
    let after = f.store.account().await.unwrap().unwrap();
    assert_eq!(before, after.material.encode().unwrap());
    assert!(vault::open_password(&after.material, &change().current_password).is_ok());
    assert!(
        vault::open_recovery(&after.material, &RecoveryKey::parse(&f.recovery).unwrap()).is_ok()
    );
}
#[tokio::test]
async fn lost_http_waiter_does_not_cancel_durable_credential_commit() {
    let f = fixtures::center().await;
    let mut db = database(&f).await;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut db)
        .await
        .unwrap();
    let service = AccountService::new(f.store.clone(), f.authority.clone());
    let lease = f.lease.clone();
    let mut revoked = lease.revocation();
    let waiter = tokio::spawn(async move { service.change_password(&lease, change()).await });
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        revoked.wait_for(|value| *value),
    )
    .await
    .unwrap()
    .unwrap();
    waiter.abort();
    let _ = waiter.await;
    sqlx::query("ROLLBACK").execute(&mut db).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if f.store.account().await.unwrap().unwrap().revision == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let after = f.store.account().await.unwrap().unwrap();
    let dek = vault::open_password(&after.material, &change().new_password).unwrap();
    f.authority.admit(after.revision, dek).unwrap();
}
#[tokio::test]
async fn corrupt_password_wrap_cannot_be_bypassed_by_matching_verifier() {
    let f = fixtures::center().await;
    let account = f.store.account().await.unwrap().unwrap();
    let mut encoded = serde_json::to_value(&account.material).unwrap();
    let nonce = encoded["password_wrap"]["nonce"][0].as_u64().unwrap();
    encoded["password_wrap"]["nonce"][0] = serde_json::json!(nonce ^ 1);
    let mut db = database(&f).await;
    sqlx::query("UPDATE account SET material=?")
        .bind(serde_json::to_vec(&encoded).unwrap())
        .execute(&mut db)
        .await
        .unwrap();
    let service = AccountService::new(f.store.clone(), f.authority.clone());
    assert!(service.change_password(&f.lease, change()).await.is_err());
    assert_eq!(f.store.account().await.unwrap().unwrap().revision, 1);
}
