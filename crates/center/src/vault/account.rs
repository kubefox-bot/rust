use super::{
    format::{Derivation, EnvelopeHeader, RecordKind},
    kdf,
    parameters::*,
    EncryptedRecord, VaultError,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "AccountWire")]
pub struct AccountMaterial {
    pub(super) format_version: u16,
    pub(super) vault_id: Uuid,
    pub(super) account_id: Uuid,
    pub(super) dek_id: Uuid,
    pub(super) verifier: String,
    pub(super) password_wrap: EncryptedRecord,
    pub(super) recovery_wrap: EncryptedRecord,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountWire {
    format_version: u16,
    vault_id: Uuid,
    account_id: Uuid,
    dek_id: Uuid,
    verifier: String,
    password_wrap: EncryptedRecord,
    recovery_wrap: EncryptedRecord,
}
impl TryFrom<AccountWire> for AccountMaterial {
    type Error = VaultError;
    fn try_from(wire: AccountWire) -> Result<Self, Self::Error> {
        let material = Self {
            format_version: wire.format_version,
            vault_id: wire.vault_id,
            account_id: wire.account_id,
            dek_id: wire.dek_id,
            verifier: wire.verifier,
            password_wrap: wire.password_wrap,
            recovery_wrap: wire.recovery_wrap,
        };
        material.validate()?;
        Ok(material)
    }
}
impl AccountMaterial {
    pub fn decode(input: &[u8]) -> Result<Self, VaultError> {
        super::encoded::decode(input, MAX_ACCOUNT_MATERIAL_BYTES)
    }
    pub fn encode(&self) -> Result<Vec<u8>, VaultError> {
        super::encoded::encode(self, MAX_ACCOUNT_MATERIAL_BYTES)
    }
    pub fn vault_id(&self) -> Uuid {
        self.vault_id
    }
    pub fn account_id(&self) -> Uuid {
        self.account_id
    }
    pub fn dek_id(&self) -> Uuid {
        self.dek_id
    }
    pub(super) fn validate(&self) -> Result<(), VaultError> {
        if self.format_version != FORMAT_VERSION
            || self.vault_id.is_nil()
            || self.account_id.is_nil()
            || self.dek_id.is_nil()
        {
            return Err(VaultError::Format);
        }
        let verifier = kdf::parse_verifier(&self.verifier)?;
        for (record, kind) in [
            (&self.password_wrap, RecordKind::PasswordWrap),
            (&self.recovery_wrap, RecordKind::RecoveryWrap),
        ] {
            let header = &record.header;
            header.validate()?;
            if header.vault_id != self.vault_id
                || header.record_id != self.account_id
                || header.record_version != ACCOUNT_RECORD_VERSION
                || header.key_id != self.dek_id
                || header.kind != kind
                || record.ciphertext.len() != KEY_BYTES + TAG_BYTES
            {
                return Err(VaultError::Format);
            }
        }
        let Derivation::Password { salt, .. } = &self.password_wrap.header.derivation else {
            return Err(VaultError::Format);
        };
        if verifier.salt.as_ref().ok_or(VaultError::Format)?.as_ref() == salt {
            return Err(VaultError::Format);
        }
        Ok(())
    }
}
pub(super) fn wrap_header(
    vault_id: Uuid,
    account_id: Uuid,
    dek_id: Uuid,
    generation: u64,
    kind: RecordKind,
    derivation: Derivation,
) -> Result<EnvelopeHeader, VaultError> {
    let header = EnvelopeHeader {
        format_version: FORMAT_VERSION,
        kind,
        cipher: CIPHER_CODE,
        vault_id,
        record_id: account_id,
        record_version: ACCOUNT_RECORD_VERSION,
        key_id: dek_id,
        key_generation: generation,
        derivation,
    };
    header.validate()?;
    Ok(header)
}
