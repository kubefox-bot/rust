use super::{
    cipher, derive,
    format::Derivation,
    kdf::{self, Arena},
    parameters::*,
    AccountMaterial, Dek, RecoveryKey, SecretText, VaultError,
};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub fn open_password(material: &AccountMaterial, password: &SecretText) -> Result<Dek, VaultError> {
    kdf::password(password)?;
    material.validate()?;
    let parsed = kdf::parse_verifier(&material.verifier)?;
    let verifier_salt: [u8; SALT_BYTES] = parsed
        .salt
        .as_ref()
        .ok_or(VaultError::Format)?
        .as_ref()
        .try_into()
        .map_err(|_| VaultError::Format)?;
    let mut arena = Arena::new()?;
    let verifier = arena.derive(password, &verifier_salt)?;
    let expected = parsed.hash.as_ref().ok_or(VaultError::Format)?.as_bytes();
    if !bool::from(verifier.as_slice().ct_eq(expected)) {
        return Err(VaultError::Authentication);
    }
    let header = &material.password_wrap.header;
    let Derivation::Password { salt, .. } = &header.derivation else {
        return Err(VaultError::Format);
    };
    let key_material = arena.derive(password, salt)?;
    drop(arena);
    let key = derive::key(
        key_material.as_slice(),
        material.vault_id.as_bytes(),
        header,
    )?;
    dek(cipher::open(&key, header, &material.password_wrap)?)
}
pub fn open_recovery(
    material: &AccountMaterial,
    recovery: &RecoveryKey,
) -> Result<Dek, VaultError> {
    material.validate()?;
    let header = &material.recovery_wrap.header;
    let Derivation::Recovery { salt, .. } = &header.derivation else {
        return Err(VaultError::Format);
    };
    let key = derive::key(recovery.0.as_slice(), salt, header)?;
    dek(cipher::open(&key, header, &material.recovery_wrap)?)
}
fn dek(bytes: Zeroizing<Vec<u8>>) -> Result<Dek, VaultError> {
    let key: [u8; KEY_BYTES] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| VaultError::Format)?;
    Ok(Dek::new(key))
}
