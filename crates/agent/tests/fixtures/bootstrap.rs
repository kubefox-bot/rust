use hmail_cleaner_adapters::mtls::PeerIdentity;
use hmail_cleaner_agent::enrollment::{self, AgentBootstrap};
use hmail_cleaner_protocol::AgentId;
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose, SanType,
};
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

pub fn prepare(root: &Path) -> (PathBuf, AgentId) {
    let directory = root.join("hMailCleanerAgent");
    let agent_id = AgentId::new();
    enrollment::prepare(&AgentBootstrap {
        data_directory: directory.clone(),
        agent_id,
    })
    .unwrap();
    let mut issuer = CertificateParams::new(Vec::<String>::new()).unwrap();
    issuer.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    issuer.key_usages = vec![KeyUsagePurpose::KeyCertSign];
    let ca = CertifiedIssuer::self_signed(issuer, KeyPair::generate().unwrap()).unwrap();
    let private =
        Zeroizing::new(std::fs::read_to_string(directory.join("identity-key.pem")).unwrap());
    let key = KeyPair::from_pem(&private).unwrap();
    let mut leaf = CertificateParams::new(vec!["agent.internal".into()]).unwrap();
    leaf.subject_alt_names.push(SanType::URI(
        PeerIdentity::Agent(agent_id.uuid())
            .uri()
            .try_into()
            .unwrap(),
    ));
    leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let identity = leaf.signed_by(&key, &ca).unwrap();
    std::fs::write(directory.join("trust.pem"), ca.pem()).unwrap();
    std::fs::write(directory.join("identity.pem"), identity.pem()).unwrap();
    let config = directory.join("agent.json");
    std::fs::write(&config, serde_json::to_vec(&serde_json::json!({ "listen": "127.0.0.1:9443", "agent_id": agent_id, "center_id": uuid::Uuid::new_v4() })).unwrap()).unwrap();
    (config, agent_id)
}
