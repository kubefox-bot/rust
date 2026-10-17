use hmail_cleaner_center::{
    account::{initialize, InitInput, SecretPrompt},
    storage::{CenterStore, DataDirectory},
    vault::SecretText,
};
struct Prompt {
    confirmed: bool,
}
impl SecretPrompt for Prompt {
    fn password(&mut self) -> Result<SecretText, String> {
        Ok(SecretText::new("test-password-long".into()))
    }
    fn confirm_recovery(&mut self, _: &SecretText) -> Result<bool, String> {
        Ok(self.confirmed)
    }
}
#[tokio::test]
async fn recovery_confirmation_precedes_commit_and_repeat_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    let store = CenterStore::open(&lease).await.unwrap();
    let input = InitInput {
        login: "admin".into(),
    };
    assert!(initialize(&store, &input, &mut Prompt { confirmed: false })
        .await
        .is_err());
    assert!(store.account().await.unwrap().is_none());
    initialize(&store, &input, &mut Prompt { confirmed: true })
        .await
        .unwrap();
    let before = store.account().await.unwrap().unwrap();
    assert!(initialize(&store, &input, &mut Prompt { confirmed: true })
        .await
        .is_err());
    assert_eq!(
        before.material.vault_id(),
        store.account().await.unwrap().unwrap().material.vault_id()
    );
}
#[tokio::test]
async fn concurrent_init_creates_exactly_one_account() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    let store = CenterStore::open(&lease).await.unwrap();
    let input = InitInput {
        login: "admin".into(),
    };
    let mut first = Prompt { confirmed: true };
    let mut second = Prompt { confirmed: true };
    let (a, b) = tokio::join!(
        initialize(&store, &input, &mut first),
        initialize(&store, &input, &mut second)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert!(store.account().await.unwrap().is_some());
}
