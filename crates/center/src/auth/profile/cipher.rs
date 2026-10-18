use crate::vault::{Entropy, SystemEntropy};
use base64ct::{Base64UrlUnpadded, Encoding};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use std::sync::Arc;
use zeroize::Zeroizing;

const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;
const MAX_TOKEN_BYTES: usize = 2048;
const TOKEN_PREFIX: &str = "v1.";
const AAD: &[u8] = b"sochi-center/header-display/__Host-sochi-profile/v1";

#[derive(Clone)]
pub(super) struct ProfileCipher(Arc<Zeroizing<[u8; KEY_BYTES]>>);
impl ProfileCipher {
    pub(super) fn new() -> Result<Self, ()> {
        let mut key = Zeroizing::new([0; KEY_BYTES]);
        SystemEntropy.fill(&mut *key).map_err(|_| ())?;
        Ok(Self(Arc::new(key)))
    }
    pub(super) fn seal(&self, plaintext: &[u8]) -> Result<String, ()> {
        if plaintext.len() > MAX_TOKEN_BYTES / 2 {
            return Err(());
        }
        let mut nonce = [0; NONCE_BYTES];
        SystemEntropy.fill(&mut nonce).map_err(|_| ())?;
        let cipher = XChaCha20Poly1305::new_from_slice(self.0.as_ref().as_ref()).map_err(|_| ())?;
        let encrypted = cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: AAD,
                },
            )
            .map_err(|_| ())?;
        let mut envelope = nonce.to_vec();
        envelope.extend(encrypted);
        let token = format!(
            "{TOKEN_PREFIX}{}",
            Base64UrlUnpadded::encode_string(&envelope)
        );
        if token.len() > MAX_TOKEN_BYTES {
            return Err(());
        }
        Ok(token)
    }
    pub(super) fn open(&self, token: &str) -> Result<Zeroizing<Vec<u8>>, ()> {
        if token.len() > MAX_TOKEN_BYTES {
            return Err(());
        }
        let encoded = token.strip_prefix(TOKEN_PREFIX).ok_or(())?;
        let envelope = Base64UrlUnpadded::decode_vec(encoded).map_err(|_| ())?;
        if envelope.len() < NONCE_BYTES + TAG_BYTES {
            return Err(());
        }
        let (nonce, encrypted) = envelope.split_at(NONCE_BYTES);
        let nonce: [u8; NONCE_BYTES] = nonce.try_into().map_err(|_| ())?;
        let cipher = XChaCha20Poly1305::new_from_slice(self.0.as_ref().as_ref()).map_err(|_| ())?;
        cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: encrypted,
                    aad: AAD,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| ())
    }
}
