use super::{format::EnvelopeHeader, parameters::*};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;
pub type SecretText = Zeroizing<String>;
pub type Dek = Zeroizing<[u8; KEY_BYTES]>;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("invalid password length")]
    Password,
    #[error("unsupported or invalid vault metadata")]
    Format,
    #[error("authentication failed")]
    Authentication,
    #[error("operating system entropy unavailable")]
    Entropy,
    #[error("cryptographic memory unavailable")]
    Resources,
    #[error("cryptographic operation failed")]
    Crypto,
    #[error("record exceeds the supported size")]
    Oversized,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedRecord {
    pub(super) header: EnvelopeHeader,
    pub(super) nonce: [u8; NONCE_BYTES],
    #[serde(with = "super::encoded")]
    pub(super) ciphertext: Vec<u8>,
}
impl EncryptedRecord {
    pub fn decode(input: &[u8]) -> Result<Self, VaultError> {
        super::encoded::decode(input, MAX_ENCRYPTED_RECORD_BYTES)
    }
    pub fn encode(&self) -> Result<Vec<u8>, VaultError> {
        super::encoded::encode(self, MAX_ENCRYPTED_RECORD_BYTES)
    }
    pub fn header(&self) -> &EnvelopeHeader {
        &self.header
    }
}
pub struct PreparedVault {
    pub material: super::account::AccountMaterial,
    pub dek: Dek,
    pub recovery: super::recovery::RecoveryKey,
}
