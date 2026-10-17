#[path = "fixtures/bootstrap.rs"]
mod fixture;
use hmail_cleaner_agent::config::AgentConfig;

#[test]
fn bootstrap_loads_local_keys_and_reports_the_same_build_as_the_package() {
    let directory = tempfile::tempdir().unwrap();
    let (path, agent, _) = fixture::signed(directory.path());
    let config = AgentConfig::load(&path, "0.1.0+fixture").unwrap();
    assert_eq!(config.agent_id, agent);
    assert_eq!(config.build_version, "0.1.0+fixture");
    assert_eq!(config.listen.port(), 9443);
    assert_eq!(config.data_directory, path.parent().unwrap());
}
#[test]
fn mismatched_id_missing_certificate_and_wrong_private_key_fail_closed() {
    let directory = tempfile::tempdir().unwrap();
    let (path, _, _) = fixture::signed(directory.path());
    let data = path.parent().unwrap();
    let original = std::fs::read(&path).unwrap();
    let mut config: serde_json::Value = serde_json::from_slice(&original).unwrap();
    config["agent_id"] = serde_json::json!(hmail_cleaner_protocol::AgentId::new());
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    assert!(AgentConfig::load(&path, "fixture").is_err());
    std::fs::write(&path, original).unwrap();
    let certificate = std::fs::read(data.join("identity.pem")).unwrap();
    std::fs::remove_file(data.join("identity.pem")).unwrap();
    assert!(AgentConfig::load(&path, "fixture").is_err());
    std::fs::write(data.join("identity.pem"), certificate).unwrap();
    let wrong_key = rcgen::KeyPair::generate().unwrap();
    std::fs::write(data.join("identity-key.pem"), wrong_key.serialize_pem()).unwrap();
    assert!(AgentConfig::load(&path, "fixture").is_err());
}
