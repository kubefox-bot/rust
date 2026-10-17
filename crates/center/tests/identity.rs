pub mod fixtures;
use hmail_cleaner_center::identity::IdentityService;
use hmail_cleaner_protocol::AgentId;
#[tokio::test]
async fn issuer_verifies_csr_and_assigns_identity_and_eku_itself() {
    let fixture = fixtures::center().await;
    let service = IdentityService::new(fixture.store.clone());
    let agent = AgentId::new();
    let (_, csr) = fixtures::csr();
    let issued = service
        .issue_agent(agent, "127.0.0.1", &csr, &fixture.lease)
        .await
        .unwrap();
    let pem = x509_parser::pem::parse_x509_pem(issued.certificate_pem.as_bytes())
        .unwrap()
        .1;
    let cert = x509_parser::parse_x509_certificate(&pem.contents)
        .unwrap()
        .1;
    assert!(!cert.raw_serial().is_empty());
    assert!(cert.raw_serial().len() <= 17);
    assert_eq!(cert.raw_serial(), issued.serial);
    assert!(!cert.is_ca());
    let sans = cert.subject_alternative_name().unwrap().unwrap();
    assert!(sans.value.general_names.iter().any(|name|matches!(name,x509_parser::extensions::GeneralName::URI(uri) if *uri==format!("urn:hmail-cleaner:agent:{agent}"))));
    let eku = cert.extended_key_usage().unwrap().unwrap();
    assert!(eku.value.server_auth);
    assert!(!eku.value.client_auth);
    assert!(service
        .issue_agent(AgentId::new(), "127.0.0.1", "not a CSR", &fixture.lease)
        .await
        .is_err());
    fixture.authority.revoke(fixture.lease.generation());
    assert!(service
        .issue_agent(AgentId::new(), "127.0.0.1", &csr, &fixture.lease)
        .await
        .is_err());
}
