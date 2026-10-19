use super::CenterFixture;
use hmail_cleaner_center::vault::{self, EnvelopeHeader, RecordIdentity, RecordKind};
#[derive(serde::Deserialize)]
pub struct CaPayload {
    pub certificate_pem: String,
    pub private_key_pem: zeroize::Zeroizing<String>,
}
pub async fn ca(center: &CenterFixture) -> CaPayload {
    let material = center.store.account().await.unwrap().unwrap().material;
    let header = EnvelopeHeader::record(RecordIdentity {
        vault_id: material.vault_id(),
        kind: RecordKind::CaKey,
        record_id: material.vault_id(),
        record_version: 1,
        key_id: material.dek_id(),
        key_generation: 1,
    })
    .unwrap();
    let record = center
        .store
        .encrypted(material.vault_id(), &header)
        .await
        .unwrap()
        .unwrap();
    let plain = center
        .lease
        .with_dek(|key| vault::open_record(key, &header, &record))
        .unwrap()
        .unwrap();
    serde_json::from_slice(&plain).unwrap()
}
pub async fn expire_agent(
    center: &CenterFixture,
    directory: &std::path::Path,
    id: hmail_cleaner_protocol::AgentId,
) {
    use rustls::pki_types::{pem::PemObject, CertificateDer};
    let ca = ca(center).await;
    let issuer = rcgen::Issuer::from_ca_cert_pem(
        &ca.certificate_pem,
        rcgen::KeyPair::from_pem(&ca.private_key_pem).unwrap(),
    )
    .unwrap();
    let private = zeroize::Zeroizing::new(
        std::fs::read_to_string(directory.join("identity-key.pem")).unwrap(),
    );
    let key = rcgen::KeyPair::from_pem(&private).unwrap();
    let current = std::fs::read(directory.join("identity.pem")).unwrap();
    let leaf = CertificateDer::from_pem_slice(&current).unwrap();
    let serial = hmail_cleaner_adapters::mtls::certificate_serial(&leaf).unwrap();
    let mut params = rcgen::CertificateParams::new(vec!["127.0.0.1".into()]).unwrap();
    params.subject_alt_names.push(rcgen::SanType::URI(
        format!("urn:hmail-cleaner:agent:{id}").try_into().unwrap(),
    ));
    params.serial_number = Some(rcgen::SerialNumber::from_slice(&serial));
    params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(2);
    params.not_after = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    let certificate = format!(
        "{}{}",
        params.signed_by(&key, &issuer).unwrap().pem(),
        ca.certificate_pem
    );
    std::fs::write(directory.join("identity.pem"), &certificate).unwrap();
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        center.root.path().join("center.sqlite").display()
    ))
    .await
    .unwrap();
    sqlx::query("UPDATE trust SET certificate_pem=?,expires_at=? WHERE agent_id=?")
        .bind(certificate)
        .bind(params.not_after.unix_timestamp())
        .bind(id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
}
