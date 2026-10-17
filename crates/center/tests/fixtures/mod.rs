use hmail_cleaner_center::{
    account::{initialize, InitInput, SecretPrompt},
    auth::{MonotonicClock, SessionAuthority, SessionLease},
    storage::{CenterStore, DataDirectory},
    vault::{self, SecretText},
};
use std::sync::Arc;
pub struct CenterFixture {
    pub root: tempfile::TempDir,
    pub store: CenterStore,
    pub authority: Arc<SessionAuthority>,
    pub lease: SessionLease,
    pub token: SecretText,
    pub recovery: SecretText,
}
struct Prompt(Option<SecretText>);
impl SecretPrompt for Prompt {
    fn password(&mut self) -> Result<SecretText, String> {
        Ok(SecretText::new("fixture-center-password".into()))
    }
    fn confirm_recovery(&mut self, key: &SecretText) -> Result<bool, String> {
        self.0 = Some(key.clone());
        Ok(true)
    }
}
pub async fn center() -> CenterFixture {
    let root = tempfile::tempdir().unwrap();
    let directory = DataDirectory::acquire(root.path()).unwrap();
    let store = CenterStore::open(&directory).await.unwrap();
    let mut prompt = Prompt(None);
    initialize(
        &store,
        &InitInput {
            login: "admin".into(),
        },
        &mut prompt,
    )
    .await
    .unwrap();
    let account = store.account().await.unwrap().unwrap();
    let dek = vault::open_password(
        &account.material,
        &SecretText::new("fixture-center-password".into()),
    )
    .unwrap();
    let authority = Arc::new(
        SessionAuthority::new(account.revision, 15, Arc::new(MonotonicClock::default())).unwrap(),
    );
    let grant = authority.admit(account.revision, dek).unwrap();
    let lease = authority.authenticate(&grant.token).unwrap();
    CenterFixture {
        root,
        store,
        authority,
        lease,
        token: grant.token,
        recovery: prompt.0.unwrap(),
    }
}
pub fn csr() -> (rcgen::KeyPair, String) {
    let key = rcgen::KeyPair::generate().unwrap();
    let request = rcgen::CertificateParams::new(Vec::<String>::new())
        .unwrap()
        .serialize_request(&key)
        .unwrap()
        .pem()
        .unwrap();
    (key, request)
}
pub mod agents;
pub mod enrollment;
pub mod identity;
pub mod job_agent;
pub mod jobs;
pub mod ssh;
