use super::parameters::*;
use base64ct::{Base64UrlUnpadded, Encoding};
use serde::{
    de::{Error, Visitor},
    Deserializer, Serializer,
};
use std::fmt;
pub(super) fn decode<T: serde::de::DeserializeOwned>(
    input: &[u8],
    maximum: usize,
) -> Result<T, super::VaultError> {
    if input.len() > maximum {
        return Err(super::VaultError::Oversized);
    }
    serde_json::from_slice(input).map_err(|_| super::VaultError::Format)
}
pub(super) fn encode<T: serde::Serialize>(
    value: &T,
    maximum: usize,
) -> Result<Vec<u8>, super::VaultError> {
    let encoded = serde_json::to_vec(value).map_err(|_| super::VaultError::Format)?;
    if encoded.len() > maximum {
        return Err(super::VaultError::Oversized);
    }
    Ok(encoded)
}
pub(super) fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&Base64UrlUnpadded::encode_string(bytes))
}
pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    struct Bounded;
    impl Visitor<'_> for Bounded {
        type Value = Vec<u8>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("bounded Base64URL ciphertext")
        }
        fn visit_str<E: Error>(self, input: &str) -> Result<Self::Value, E> {
            if input.len() > MAX_ENCODED_BYTES {
                return Err(E::custom("ciphertext exceeds limit"));
            }
            let bytes = Base64UrlUnpadded::decode_vec(input)
                .map_err(|_| E::custom("invalid ciphertext encoding"))?;
            if !(TAG_BYTES..=MAX_CIPHERTEXT_BYTES).contains(&bytes.len()) {
                return Err(E::custom("invalid ciphertext size"));
            }
            Ok(bytes)
        }
    }
    deserializer.deserialize_str(Bounded)
}
