use super::{
    account, cipher, derive, entropy,
    format::{ArgonProfile, Derivation, RecordKind},
    kdf::{self, Arena},
    parameters::*,
    AccountMaterial, Dek, Entropy, RecoveryKey, SecretText, VaultError,
};

pub(crate) fn replace_password(
    material: &AccountMaterial,
    dek: &Dek,
    password: &SecretText,
    source: &dyn Entropy,
) -> Result<AccountMaterial, VaultError> {
    kdf::password(password)?;
    material.validate()?;
    let verifier_salt = entropy::bytes::<SALT_BYTES>(source)?;
    let salt = entropy::bytes::<SALT_BYTES>(source)?;
    if verifier_salt == salt {
        return Err(VaultError::Entropy);
    }
    let generation = material
        .password_wrap
        .header
        .key_generation
        .checked_add(1)
        .ok_or(VaultError::Format)?;
    let header = account::wrap_header(
        material.vault_id,
        material.account_id,
        material.dek_id,
        generation,
        RecordKind::PasswordWrap,
        Derivation::Password {
            account_id: material.account_id,
            profile: ArgonProfile::supported(),
            salt,
        },
    )?;
    let mut arena = Arena::new()?;
    let verifier_hash = arena.derive(password, &verifier_salt)?;
    let verifier = kdf::verifier(&verifier_salt, &verifier_hash)?;
    let derived = arena.derive(password, &salt)?;
    drop(arena);
    let key = derive::key(derived.as_slice(), material.vault_id.as_bytes(), &header)?;
    let password_wrap = cipher::seal(&key, &header, dek.as_slice(), source)?;
    let result = AccountMaterial {
        verifier,
        password_wrap,
        ..material.clone()
    };
    result.validate()?;
    Ok(result)
}

pub(crate) fn replace_recovery(
    material: &AccountMaterial,
    dek: &Dek,
    recovery: &RecoveryKey,
    source: &dyn Entropy,
) -> Result<AccountMaterial, VaultError> {
    material.validate()?;
    let salt = entropy::bytes::<RECOVERY_SALT_BYTES>(source)?;
    let generation = material
        .recovery_wrap
        .header
        .key_generation
        .checked_add(1)
        .ok_or(VaultError::Format)?;
    let header = account::wrap_header(
        material.vault_id,
        material.account_id,
        material.dek_id,
        generation,
        RecordKind::RecoveryWrap,
        Derivation::Recovery {
            account_id: material.account_id,
            algorithm: HKDF_ALGORITHM_CODE,
            salt,
        },
    )?;
    let key = derive::key(recovery.0.as_slice(), &salt, &header)?;
    let recovery_wrap = cipher::seal(&key, &header, dek.as_slice(), source)?;
    let result = AccountMaterial {
        recovery_wrap,
        ..material.clone()
    };
    result.validate()?;
    Ok(result)
}
