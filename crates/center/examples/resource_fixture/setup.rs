use crate::fixtures::CenterFixture;
use hmail_cleaner_center::{
    account::{initialize, InitInput, SecretPrompt},
    auth::{MonotonicClock, SessionAuthority},
    profiles::{ProfileService, ServerRegistration},
    storage::{CenterStore, DataDirectory},
    vault::{self, SecretText},
};
use hmail_cleaner_protocol::{
    browser::profiles::{PasswordPatch, ProfilePatch},
    AgentId, DatabaseTls, ProfileId, ServerId,
};
use std::sync::Arc;

const FIXTURE_PASSWORD: &str = "fixture-center-password";
struct Prompt(Option<SecretText>);
impl SecretPrompt for Prompt {
    fn password(&mut self) -> Result<SecretText, String> {
        Ok(SecretText::new(FIXTURE_PASSWORD.into()))
    }
    fn confirm_recovery(&mut self, value: &SecretText) -> Result<bool, String> {
        self.0 = Some(value.clone());
        Ok(true)
    }
}
pub async fn center(parent: &std::path::Path) -> CenterFixture {
    let root = tempfile::Builder::new()
        .prefix("hmail-mvp-synthetic-resource-")
        .tempdir_in(parent)
        .unwrap();
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
    let dek =
        vault::open_password(&account.material, &SecretText::new(FIXTURE_PASSWORD.into())).unwrap();
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
pub async fn server(
    center: &CenterFixture,
    id: AgentId,
    endpoint: &str,
    ordinal: &str,
) -> ServerRegistration {
    let target = ServerRegistration {
        server_id: ServerId::new(),
        profile_id: ProfileId::new(),
        agent_id: id,
        name: format!("Mail {ordinal}"),
        endpoint: endpoint.into(),
    };
    let profiles = ProfileService::new(center.store.clone());
    profiles
        .register(
            ServerRegistration {
                server_id: target.server_id,
                profile_id: target.profile_id,
                agent_id: id,
                name: target.name.clone(),
                endpoint: target.endpoint.clone(),
            },
            &center.lease,
        )
        .await
        .unwrap();
    profiles
        .save(
            target.profile_id,
            1,
            ProfilePatch {
                mysql_host: "127.0.0.1".into(),
                mysql_port: 3306,
                mysql_database: "synthetic_only".into(),
                mysql_username: "fixture".into(),
                password: PasswordPatch::Replace(zeroize::Zeroizing::new(
                    "synthetic-only-fixture-password".into(),
                )),
                mysql_tls: DatabaseTls::DisabledLocal,
                data_dir: format!(r"C:\hmail-mvp-synthetic\{ordinal}\mail"),
                hmail_ini: format!(r"C:\hmail-mvp-synthetic\{ordinal}\hMailServer.ini"),
                service_name: format!("hmail-mvp-fixture-{ordinal}"),
            },
            &center.lease,
        )
        .await
        .unwrap();
    target
}
