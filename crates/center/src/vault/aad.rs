use super::{
    format::{Derivation, EnvelopeHeader},
    parameters::*,
    VaultError,
};

pub fn encode_aad(header: &EnvelopeHeader) -> Result<Vec<u8>, VaultError> {
    header.validate()?;
    let mut bytes = Vec::with_capacity(MAX_AAD_BYTES);
    bytes.extend_from_slice(AAD_DOMAIN);
    bytes.extend_from_slice(&header.format_version.to_be_bytes());
    bytes.push(header.kind as u8);
    bytes.push(header.cipher);
    bytes.extend_from_slice(header.vault_id.as_bytes());
    bytes.extend_from_slice(header.record_id.as_bytes());
    bytes.extend_from_slice(&header.record_version.to_be_bytes());
    bytes.extend_from_slice(header.key_id.as_bytes());
    bytes.extend_from_slice(&header.key_generation.to_be_bytes());
    match &header.derivation {
        Derivation::Record => bytes.push(DERIVATION_RECORD_CODE),
        Derivation::Password {
            account_id,
            profile,
            salt,
        } => {
            bytes.push(DERIVATION_PASSWORD_CODE);
            bytes.extend_from_slice(account_id.as_bytes());
            bytes.push(profile.algorithm);
            for value in [
                profile.version,
                profile.memory_kib,
                profile.passes,
                profile.lanes,
            ] {
                bytes.extend_from_slice(&value.to_be_bytes());
            }
            bytes.extend_from_slice(&profile.output_bytes.to_be_bytes());
            bytes.extend_from_slice(&(SALT_BYTES as u16).to_be_bytes());
            bytes.extend_from_slice(salt);
        }
        Derivation::Recovery {
            account_id,
            algorithm,
            salt,
        } => {
            bytes.push(DERIVATION_RECOVERY_CODE);
            bytes.extend_from_slice(account_id.as_bytes());
            bytes.push(*algorithm);
            bytes.extend_from_slice(&(RECOVERY_SALT_BYTES as u16).to_be_bytes());
            bytes.extend_from_slice(salt);
        }
    }
    Ok(bytes)
}
