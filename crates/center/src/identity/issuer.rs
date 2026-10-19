use super::{material::CaSecret, IdentityError};
use crate::{
    auth::SessionLease,
    storage::CenterStore,
    vault::{self, AccountMaterial, RecordKind},
};
use rcgen::{Issuer, KeyPair};
use rustls::pki_types::{pem::PemObject, CertificateDer};
pub(super) struct SigningIdentity {
    pub issuer: Issuer<'static, KeyPair>,
    pub ca_pem: String,
}
pub(super) async fn load(
    store: &CenterStore,
    material: &AccountMaterial,
    lease: &SessionLease,
) -> Result<SigningIdentity, IdentityError> {
    let header = super::policy::header(material, RecordKind::CaKey, material.vault_id(), 1)?;
    let record = store
        .encrypted(material.vault_id(), &header)
        .await
        .map_err(|_| IdentityError)?
        .ok_or(IdentityError)?;
    let plaintext = lease
        .with_dek(|key| vault::open_record(key, &header, &record))
        .map_err(|_| IdentityError)?
        .map_err(|_| IdentityError)?;
    let secret: CaSecret = serde_json::from_slice(&plaintext).map_err(|_| IdentityError)?;
    let key = KeyPair::from_pem(&secret.private_key_pem).map_err(|_| IdentityError)?;
    let der = CertificateDer::from_pem_slice(secret.certificate_pem.as_bytes())
        .map_err(|_| IdentityError)?;
    let (remaining, certificate) =
        x509_parser::parse_x509_certificate(&der).map_err(|_| IdentityError)?;
    if !remaining.is_empty()
        || !certificate.is_ca()
        || !certificate.validity().is_valid()
        || certificate.public_key().subject_public_key.data.as_ref() != key.public_key_raw()
        || !certificate
            .key_usage()
            .map_err(|_| IdentityError)?
            .is_some_and(|usage| usage.value.key_cert_sign())
    {
        return Err(IdentityError);
    }
    certificate
        .verify_signature(Some(certificate.public_key()))
        .map_err(|_| IdentityError)?;
    let issuer =
        Issuer::from_ca_cert_pem(&secret.certificate_pem, key).map_err(|_| IdentityError)?;
    Ok(SigningIdentity {
        issuer,
        ca_pem: secret.certificate_pem,
    })
}
