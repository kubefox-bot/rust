use super::VaultError;
use ring::rand::{SecureRandom, SystemRandom};
use uuid::Uuid;
pub trait Entropy: Send + Sync {
    fn fill(&self, destination: &mut [u8]) -> Result<(), VaultError>;
}
pub struct SystemEntropy;
impl Entropy for SystemEntropy {
    fn fill(&self, destination: &mut [u8]) -> Result<(), VaultError> {
        SystemRandom::new()
            .fill(destination)
            .map_err(|_| VaultError::Entropy)
    }
}
pub(super) fn bytes<const N: usize>(entropy: &dyn Entropy) -> Result<[u8; N], VaultError> {
    let mut bytes = [0; N];
    entropy.fill(&mut bytes)?;
    Ok(bytes)
}
pub(super) fn identifier(entropy: &dyn Entropy) -> Result<Uuid, VaultError> {
    Ok(uuid::Builder::from_random_bytes(bytes(entropy)?).into_uuid())
}
