use super::{
    account::{self, AccountMaterial},
    cipher, derive, entropy,
    format::{ArgonProfile, Derivation, RecordKind},
    kdf::{self, Arena},
    parameters::*,
    Dek, Entropy, PreparedVault, RecoveryKey, SecretText, VaultError,
};

pub fn initialize(
    password: &SecretText,
    source: &dyn Entropy,
) -> Result<PreparedVault, VaultError> {
    kdf::password(password)?;
    let vault_id = entropy::identifier(source)?;
    let account_id = entropy::identifier(source)?;
    let dek_id = entropy::identifier(source)?;
    let dek = Dek::new(entropy::bytes(source)?);
    let recovery = RecoveryKey::generate(source)?;
    let verifier_salt = entropy::bytes::<SALT_BYTES>(source)?;
    let password_salt = entropy::bytes::<SALT_BYTES>(source)?;
    if verifier_salt == password_salt {
        return Err(VaultError::Entropy);
    }
    let recovery_salt = entropy::bytes::<RECOVERY_SALT_BYTES>(source)?;
    let password_header = account::wrap_header(
        vault_id,
        account_id,
        dek_id,
        INITIAL_GENERATION,
        RecordKind::PasswordWrap,
        Derivation::Password {
            account_id,
            profile: ArgonProfile::supported(),
            salt: password_salt,
        },
    )?;
    let recovery_header = account::wrap_header(
        vault_id,
        account_id,
        dek_id,
        INITIAL_GENERATION,
        RecordKind::RecoveryWrap,
        Derivation::Recovery {
            account_id,
            algorithm: HKDF_ALGORITHM_CODE,
            salt: recovery_salt,
        },
    )?;
    let mut arena = Arena::new()?;
    let verifier_hash = arena.derive(password, &verifier_salt)?;
    let verifier = kdf::verifier(&verifier_salt, &verifier_hash)?;
    let password_material = arena.derive(password, &password_salt)?;
    drop(arena);
    let password_key = derive::key(
        password_material.as_slice(),
        vault_id.as_bytes(),
        &password_header,
    )?;
    let recovery_key = derive::key(recovery.0.as_slice(), &recovery_salt, &recovery_header)?;
    let password_wrap = cipher::seal(&password_key, &password_header, dek.as_slice(), source)?;
    let recovery_wrap = cipher::seal(&recovery_key, &recovery_header, dek.as_slice(), source)?;
    let material = AccountMaterial {
        format_version: FORMAT_VERSION,
        vault_id,
        account_id,
        dek_id,
        verifier,
        password_wrap,
        recovery_wrap,
    };
    material.validate()?;
    Ok(PreparedVault {
        material,
        dek,
        recovery,
    })
}
