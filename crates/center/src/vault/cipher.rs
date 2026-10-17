use super::{
    aad::encode_aad, derive, entropy, parameters::*, Dek, EncryptedRecord, Entropy, EnvelopeHeader,
    SystemEntropy, VaultError,
};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use zeroize::Zeroizing;

pub fn seal_record(
    dek: &Dek,
    header: &EnvelopeHeader,
    plaintext: &[u8],
) -> Result<EncryptedRecord, VaultError> {
    if !header.kind.is_record() {
        return Err(VaultError::Format);
    }
    let key = derive::key(dek.as_slice(), header.vault_id.as_bytes(), header)?;
    seal(&key, header, plaintext, &SystemEntropy)
}
pub fn open_record(
    dek: &Dek,
    expected: &EnvelopeHeader,
    record: &EncryptedRecord,
) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    if !expected.kind.is_record() {
        return Err(VaultError::Format);
    }
    let key = derive::key(dek.as_slice(), expected.vault_id.as_bytes(), expected)?;
    open(&key, expected, record)
}
pub(super) fn seal(
    key: &[u8; KEY_BYTES],
    header: &EnvelopeHeader,
    plaintext: &[u8],
    source: &dyn Entropy,
) -> Result<EncryptedRecord, VaultError> {
    if plaintext.len() > MAX_PLAINTEXT_BYTES {
        return Err(VaultError::Oversized);
    }
    let aad = encode_aad(header)?;
    let nonce = entropy::bytes::<NONCE_BYTES>(source)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| VaultError::Crypto)?;
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| VaultError::Crypto)?;
    Ok(EncryptedRecord {
        header: header.clone(),
        nonce,
        ciphertext,
    })
}
pub(super) fn open(
    key: &[u8; KEY_BYTES],
    expected: &EnvelopeHeader,
    record: &EncryptedRecord,
) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    if expected != &record.header {
        return Err(VaultError::Authentication);
    }
    if !(TAG_BYTES..=MAX_CIPHERTEXT_BYTES).contains(&record.ciphertext.len()) {
        return Err(VaultError::Oversized);
    }
    let aad = encode_aad(expected)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| VaultError::Crypto)?;
    cipher
        .decrypt(
            &XNonce::from(record.nonce),
            Payload {
                msg: &record.ciphertext,
                aad: &aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| VaultError::Authentication)
}
