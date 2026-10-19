use super::{parameters::*, VaultError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum RecordKind {
    Profile = 1,
    CaKey = 2,
    CenterClientKey = 3,
    PasswordWrap = 4,
    RecoveryWrap = 5,
}
impl RecordKind {
    pub(super) fn label(self) -> &'static [u8] {
        match self {
            Self::Profile => b"profile",
            Self::CaKey => b"ca-key",
            Self::CenterClientKey => b"center-client-key",
            Self::PasswordWrap => b"password-wrap",
            Self::RecoveryWrap => b"recovery-wrap",
        }
    }
    pub(super) fn is_record(self) -> bool {
        match self {
            Self::Profile | Self::CaKey | Self::CenterClientKey => true,
            Self::PasswordWrap | Self::RecoveryWrap => false,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct RecordIdentity {
    pub vault_id: Uuid,
    pub kind: RecordKind,
    pub record_id: Uuid,
    pub record_version: u64,
    pub key_id: Uuid,
    pub key_generation: u64,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ArgonProfile {
    pub algorithm: u8,
    pub version: u32,
    pub memory_kib: u32,
    pub passes: u32,
    pub lanes: u32,
    pub output_bytes: u16,
}
impl ArgonProfile {
    pub fn supported() -> Self {
        Self {
            algorithm: ARGON_ALGORITHM_CODE,
            version: ARGON_VERSION,
            memory_kib: ARGON_MEMORY_KIB,
            passes: ARGON_PASSES,
            lanes: ARGON_LANES,
            output_bytes: KEY_BYTES as u16,
        }
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Derivation {
    Record,
    Password {
        account_id: Uuid,
        profile: ArgonProfile,
        salt: [u8; SALT_BYTES],
    },
    Recovery {
        account_id: Uuid,
        algorithm: u8,
        salt: [u8; RECOVERY_SALT_BYTES],
    },
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "HeaderWire")]
pub struct EnvelopeHeader {
    pub(super) format_version: u16,
    pub(super) kind: RecordKind,
    pub(super) cipher: u8,
    pub(super) vault_id: Uuid,
    pub(super) record_id: Uuid,
    pub(super) record_version: u64,
    pub(super) key_id: Uuid,
    pub(super) key_generation: u64,
    pub(super) derivation: Derivation,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeaderWire {
    format_version: u16,
    kind: RecordKind,
    cipher: u8,
    vault_id: Uuid,
    record_id: Uuid,
    record_version: u64,
    key_id: Uuid,
    key_generation: u64,
    derivation: Derivation,
}
impl TryFrom<HeaderWire> for EnvelopeHeader {
    type Error = VaultError;
    fn try_from(wire: HeaderWire) -> Result<Self, Self::Error> {
        let header = Self {
            format_version: wire.format_version,
            kind: wire.kind,
            cipher: wire.cipher,
            vault_id: wire.vault_id,
            record_id: wire.record_id,
            record_version: wire.record_version,
            key_id: wire.key_id,
            key_generation: wire.key_generation,
            derivation: wire.derivation,
        };
        header.validate()?;
        Ok(header)
    }
}
impl EnvelopeHeader {
    pub fn record(identity: RecordIdentity) -> Result<Self, VaultError> {
        if !identity.kind.is_record() {
            return Err(VaultError::Format);
        }
        let header = Self {
            format_version: FORMAT_VERSION,
            kind: identity.kind,
            cipher: CIPHER_CODE,
            vault_id: identity.vault_id,
            record_id: identity.record_id,
            record_version: identity.record_version,
            key_id: identity.key_id,
            key_generation: identity.key_generation,
            derivation: Derivation::Record,
        };
        header.validate()?;
        Ok(header)
    }
    pub fn record_id(&self) -> Uuid {
        self.record_id
    }
    pub fn record_version(&self) -> u64 {
        self.record_version
    }
    pub fn kind(&self) -> RecordKind {
        self.kind
    }
    pub fn vault_id(&self) -> Uuid {
        self.vault_id
    }
    pub fn key_id(&self) -> Uuid {
        self.key_id
    }
    pub fn key_generation(&self) -> u64 {
        self.key_generation
    }
    pub(super) fn validate(&self) -> Result<(), VaultError> {
        if self.format_version != FORMAT_VERSION
            || self.cipher != CIPHER_CODE
            || self.vault_id.is_nil()
            || self.record_id.is_nil()
            || self.key_id.is_nil()
            || self.record_version == 0
            || self.key_generation == 0
        {
            return Err(VaultError::Format);
        }
        let valid = match (&self.kind, &self.derivation) {
            (
                RecordKind::Profile | RecordKind::CaKey | RecordKind::CenterClientKey,
                Derivation::Record,
            ) => true,
            (
                RecordKind::PasswordWrap,
                Derivation::Password {
                    account_id,
                    profile,
                    ..
                },
            ) => *account_id == self.record_id && *profile == ArgonProfile::supported(),
            (
                RecordKind::RecoveryWrap,
                Derivation::Recovery {
                    account_id,
                    algorithm,
                    ..
                },
            ) => *account_id == self.record_id && *algorithm == HKDF_ALGORITHM_CODE,
            _ => false,
        };
        if !valid {
            return Err(VaultError::Format);
        }
        Ok(())
    }
}
