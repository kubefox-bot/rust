use crate::{enrollment::EnrollmentError, vault::SecretText};
use russh::keys::{ssh_key::Kdf, PrivateKey};
const MAX_BCRYPT_ROUNDS: u32 = 64;
const MAX_KDF_SALT_BYTES: usize = 64;
static KEY_KDF: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
pub(super) async fn load(
    input: SecretText,
    passphrase: Option<SecretText>,
) -> Result<PrivateKey, EnrollmentError> {
    let permit = KEY_KDF.try_acquire().map_err(|_| EnrollmentError::Busy)?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        // Parse OpenSSH metadata before starting bcrypt; arbitrary caller-supplied costs are refused.
        let key = PrivateKey::from_openssh(input.as_str())
            .map_err(|_| EnrollmentError::Authentication)?;
        match key.kdf() {
            Kdf::None if !key.is_encrypted() => Ok(key),
            Kdf::Bcrypt { salt, rounds }
                if (1..=MAX_BCRYPT_ROUNDS).contains(rounds) && salt.len() <= MAX_KDF_SALT_BYTES =>
            {
                key.decrypt(passphrase.ok_or(EnrollmentError::Authentication)?.as_str())
                    .map_err(|_| EnrollmentError::Authentication)
            }
            _ => Err(EnrollmentError::Authentication),
        }
    })
    .await
    .map_err(|_| EnrollmentError::Authentication)?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn openssh_key_load_preserves_identity_and_bounded_decryption() {
        use russh::keys::{ssh_key::LineEnding, Algorithm};
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
        let loaded = load(key.to_openssh(LineEnding::LF).unwrap(), None)
            .await
            .unwrap();
        assert_eq!(loaded.public_key(), key.public_key());
        let encrypted = key
            .encrypt(&mut rand::rng(), b"fixture-passphrase")
            .unwrap();
        let loaded = load(
            encrypted.to_openssh(LineEnding::LF).unwrap(),
            Some(SecretText::new("fixture-passphrase".into())),
        )
        .await
        .unwrap();
        assert_eq!(loaded.public_key(), key.public_key());
    }
}
