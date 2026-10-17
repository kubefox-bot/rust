use super::{format::EnvelopeHeader, parameters::*, VaultError};
use ring::hkdf;
use zeroize::Zeroizing;
struct KeyLength;
impl hkdf::KeyType for KeyLength {
    fn len(&self) -> usize {
        KEY_BYTES
    }
}

pub(super) fn key(
    input: &[u8],
    salt: &[u8],
    header: &EnvelopeHeader,
) -> Result<Zeroizing<[u8; KEY_BYTES]>, VaultError> {
    let mut info = HKDF_DOMAIN.to_vec();
    if header.kind.is_record() {
        info.extend_from_slice(b"record/");
    }
    info.extend_from_slice(header.kind.label());
    info.push(b'/');
    info.extend_from_slice(header.record_id.as_bytes());
    info.extend_from_slice(header.vault_id.as_bytes());
    info.extend_from_slice(header.key_id.as_bytes());
    info.extend_from_slice(&header.key_generation.to_be_bytes());
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, salt).extract(input);
    let binding = [info.as_slice()];
    let expanded = prk
        .expand(&binding, KeyLength)
        .map_err(|_| VaultError::Crypto)?;
    let mut output = Zeroizing::new([0; KEY_BYTES]);
    expanded
        .fill(output.as_mut_slice())
        .map_err(|_| VaultError::Crypto)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::{RecordIdentity, RecordKind};
    use uuid::Uuid;
    #[test]
    fn record_keys_are_separated_by_purpose_identity_and_generation() {
        let vault = Uuid::from_u128(1);
        let record = Uuid::from_u128(2);
        let key_id = Uuid::from_u128(3);
        let header = |kind, record_id, key_generation| {
            EnvelopeHeader::record(RecordIdentity {
                vault_id: vault,
                kind,
                record_id,
                record_version: 1,
                key_id,
                key_generation,
            })
            .unwrap()
        };
        let profile = header(RecordKind::Profile, record, 1);
        let first = key(&[42; KEY_BYTES], vault.as_bytes(), &profile).unwrap();
        for other in [
            header(RecordKind::CaKey, record, 1),
            header(RecordKind::CenterClientKey, record, 1),
            header(RecordKind::Profile, Uuid::from_u128(4), 1),
            header(RecordKind::Profile, record, 2),
        ] {
            let other_key = key(&[42; KEY_BYTES], vault.as_bytes(), &other).unwrap();
            assert!(first.as_slice() != other_key.as_slice());
        }
    }
}
