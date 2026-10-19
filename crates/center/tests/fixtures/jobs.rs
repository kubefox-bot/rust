use super::CenterFixture;
use hmail_cleaner_center::{
    identity::IdentityService,
    profiles::{ProfileService, ServerRegistration},
};
use hmail_cleaner_protocol::{
    browser::profiles::{PasswordPatch, ProfilePatch},
    AgentId, DatabaseTls, ProfileId, ResolvedCutoff, ServerId,
};
pub async fn register(center: &CenterFixture, endpoint: &str) -> ServerRegistration {
    let id = AgentId::new();
    let (_, csr) = super::csr();
    IdentityService::new(center.store.clone())
        .issue_agent(id, "127.0.0.1", &csr, &center.lease)
        .await
        .unwrap();
    configured(center, id, endpoint).await
}
pub async fn configured(center: &CenterFixture, id: AgentId, endpoint: &str) -> ServerRegistration {
    configured_named(center, id, endpoint, "Mail fixture").await
}
pub async fn configured_named(
    center: &CenterFixture,
    id: AgentId,
    endpoint: &str,
    name: &str,
) -> ServerRegistration {
    let target = ServerRegistration {
        server_id: ServerId::new(),
        profile_id: ProfileId::new(),
        agent_id: id,
        name: name.into(),
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
                mysql_database: "hmail".into(),
                mysql_username: "mail".into(),
                password: PasswordPatch::Replace(zeroize::Zeroizing::new(
                    "NEVER-PERSIST-JOB-PASSWORD".into(),
                )),
                mysql_tls: DatabaseTls::DisabledLocal,
                data_dir: r"C:\Mail Data".into(),
                hmail_ini: r"C:\hMailServer\hMailServer.ini".into(),
                service_name: "hMailServer".into(),
            },
            &center.lease,
        )
        .await
        .unwrap();
    target
}
pub fn cutoff() -> ResolvedCutoff {
    let utc = "2026-01-01T00:00:00Z"
        .parse::<chrono::DateTime<chrono::Utc>>()
        .unwrap();
    ResolvedCutoff {
        local: utc.naive_utc(),
        file_cutoff_utc: utc,
        utc_offset_seconds: 0,
        resolved_at_utc: utc,
    }
}
