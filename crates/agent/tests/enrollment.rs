use hmail_cleaner_agent::enrollment::{prepare, AgentBootstrap};
use hmail_cleaner_protocol::AgentId;

#[test]
fn agent_private_key_stays_local_and_repeated_enrollment_preserves_identity() {
    let directory = tempfile::tempdir().unwrap();
    let bootstrap = AgentBootstrap {
        data_directory: directory.path().join("agent"),
        agent_id: AgentId::new(),
    };
    let first = prepare(&bootstrap).unwrap();
    assert_eq!(first.agent_id, bootstrap.agent_id);
    assert!(first.csr_pem.contains("BEGIN CERTIFICATE REQUEST"));
    assert!(!first.csr_pem.contains("PRIVATE KEY"));
    let before = std::fs::read(bootstrap.data_directory.join("identity-key.pem")).unwrap();
    let repeated = prepare(&bootstrap).unwrap();
    assert_eq!(repeated.agent_id, first.agent_id);
    assert!(before == std::fs::read(bootstrap.data_directory.join("identity-key.pem")).unwrap());
    let wrong = AgentBootstrap {
        agent_id: AgentId::new(),
        ..bootstrap
    };
    assert!(prepare(&wrong).is_err());
}

#[test]
fn partial_enrollment_does_not_overwrite_a_private_key() {
    let directory = tempfile::tempdir().unwrap();
    let bootstrap = AgentBootstrap {
        data_directory: directory.path().join("agent"),
        agent_id: AgentId::new(),
    };
    prepare(&bootstrap).unwrap();
    let key = std::fs::read(bootstrap.data_directory.join("identity-key.pem")).unwrap();
    std::fs::remove_file(bootstrap.data_directory.join("agent-id.json")).unwrap();
    assert!(matches!(
        prepare(&bootstrap),
        Err(hmail_cleaner_agent::enrollment::EnrollmentError::Partial)
    ));
    assert!(std::fs::read(bootstrap.data_directory.join("identity-key.pem")).unwrap() == key);
}
