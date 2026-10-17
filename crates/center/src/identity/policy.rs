use super::IdentityError;
use crate::vault::{Entropy, SystemEntropy};
pub(super) const IDENTITY_DAYS: i64 = 90;
pub(super) fn serial() -> Result<rcgen::SerialNumber, IdentityError> {
    let mut bytes = [0; 16];
    SystemEntropy.fill(&mut bytes).map_err(|_| IdentityError)?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(IdentityError);
    }
    Ok(rcgen::SerialNumber::from_slice(&bytes))
}
pub(super) fn header(
    material: &crate::vault::AccountMaterial,
    kind: crate::vault::RecordKind,
    id: uuid::Uuid,
    version: u64,
) -> Result<crate::vault::EnvelopeHeader, IdentityError> {
    crate::vault::EnvelopeHeader::record(crate::vault::RecordIdentity {
        vault_id: material.vault_id(),
        kind,
        record_id: id,
        record_version: version,
        key_id: material.dek_id(),
        key_generation: 1,
    })
    .map_err(|_| IdentityError)
}
pub(super) const CLOCK_SKEW_MINUTES: i64 = 5;
pub(super) const EXPIRY_WARNING_DAYS: i64 = 14;
pub(super) fn client_params(center: uuid::Uuid) -> Result<rcgen::CertificateParams, IdentityError> {
    use rcgen::{CertificateParams, ExtendedKeyUsagePurpose, KeyUsagePurpose, SanType};
    let now = time::OffsetDateTime::now_utc();
    let mut params = CertificateParams::new(Vec::<String>::new()).map_err(|_| IdentityError)?;
    params.subject_alt_names.push(SanType::URI(
        format!("urn:hmail-cleaner:center:{center}")
            .try_into()
            .map_err(|_| IdentityError)?,
    ));
    params.serial_number = Some(serial()?);
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    params.not_before = now - time::Duration::minutes(CLOCK_SKEW_MINUTES);
    params.not_after = now + time::Duration::days(IDENTITY_DAYS);
    Ok(params)
}
