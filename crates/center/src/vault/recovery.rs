use super::{entropy, parameters::*, Entropy, SecretText, VaultError};
use base64ct::{Base64UrlUnpadded, Encoding};
use zeroize::Zeroizing;
pub struct RecoveryKey(pub(super) Zeroizing<[u8; KEY_BYTES]>);
impl RecoveryKey {
    pub fn generate(source: &dyn Entropy) -> Result<Self, VaultError> {
        Ok(Self(Zeroizing::new(entropy::bytes(source)?)))
    }
    pub fn expose(&self) -> SecretText {
        let mut encoded = SecretText::new(RECOVERY_PREFIX.to_owned());
        let payload = SecretText::new(Base64UrlUnpadded::encode_string(self.0.as_slice()));
        encoded.push_str(&payload);
        encoded
    }
    pub fn parse(input: &SecretText) -> Result<Self, VaultError> {
        let payload = input
            .strip_prefix(RECOVERY_PREFIX)
            .ok_or(VaultError::Format)?;
        if payload.len() != RECOVERY_ENCODED_BYTES {
            return Err(VaultError::Format);
        }
        let mut bytes = Zeroizing::new([0; KEY_BYTES]);
        let length = Base64UrlUnpadded::decode(payload, bytes.as_mut_slice())
            .map_err(|_| VaultError::Format)?
            .len();
        if length != KEY_BYTES {
            return Err(VaultError::Format);
        }
        Ok(Self(bytes))
    }
}
