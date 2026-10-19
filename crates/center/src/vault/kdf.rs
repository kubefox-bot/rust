use super::{parameters::*, SecretText, VaultError};
use argon2::{
    password_hash::phc::{Output, ParamsString, Salt},
    Algorithm, Argon2, Block, Params, PasswordHash, Version, ARGON2ID_IDENT,
};
use zeroize::Zeroizing;

pub(super) struct Arena(Zeroizing<Vec<Block>>);
impl Arena {
    pub fn new() -> Result<Self, VaultError> {
        let count = ARGON_MEMORY_KIB as usize;
        let mut blocks = Vec::new();
        blocks
            .try_reserve_exact(count)
            .map_err(|_| VaultError::Resources)?;
        blocks.resize(count, Block::new());
        Ok(Self(Zeroizing::new(blocks)))
    }
    pub fn derive(
        &mut self,
        password: &SecretText,
        salt: &[u8; SALT_BYTES],
    ) -> Result<Zeroizing<[u8; KEY_BYTES]>, VaultError> {
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params()?);
        let mut output = Zeroizing::new([0; KEY_BYTES]);
        argon
            .hash_password_into_with_memory(
                password.as_bytes(),
                salt,
                output.as_mut_slice(),
                self.0.as_mut_slice(),
            )
            .map_err(|_| VaultError::Crypto)?;
        Ok(output)
    }
}
fn params() -> Result<Params, VaultError> {
    Params::new(ARGON_MEMORY_KIB, ARGON_PASSES, ARGON_LANES, Some(KEY_BYTES))
        .map_err(|_| VaultError::Format)
}
pub(super) fn password(input: &SecretText) -> Result<(), VaultError> {
    if input.len() > MAX_PASSWORD_BYTES
        || !(MIN_PASSWORD_CHARACTERS..=MAX_PASSWORD_CHARACTERS).contains(&input.chars().count())
    {
        return Err(VaultError::Password);
    }
    Ok(())
}
pub(super) fn verifier(
    salt: &[u8; SALT_BYTES],
    hash: &[u8; KEY_BYTES],
) -> Result<String, VaultError> {
    Ok(PasswordHash {
        algorithm: ARGON2ID_IDENT,
        version: Some(ARGON_VERSION),
        params: ParamsString::try_from(params()?).map_err(|_| VaultError::Format)?,
        salt: Some(Salt::new(salt).map_err(|_| VaultError::Format)?),
        hash: Some(Output::new(hash).map_err(|_| VaultError::Format)?),
    }
    .to_string())
}
pub(super) fn parse_verifier(input: &str) -> Result<PasswordHash, VaultError> {
    if input.len() > MAX_VERIFIER_BYTES {
        return Err(VaultError::Format);
    }
    let parsed = PasswordHash::new(input).map_err(|_| VaultError::Format)?;
    if parsed.algorithm != ARGON2ID_IDENT
        || parsed.version != Some(ARGON_VERSION)
        || Params::try_from(&parsed).map_err(|_| VaultError::Format)? != params()?
        || parsed
            .salt
            .as_ref()
            .is_none_or(|salt| salt.len() != SALT_BYTES)
        || parsed
            .hash
            .as_ref()
            .is_none_or(|hash| hash.len() != KEY_BYTES)
    {
        return Err(VaultError::Format);
    }
    Ok(parsed)
}
