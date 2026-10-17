mod bootstrap;
mod issue;
mod issuer;
mod material;
mod policy;
pub(crate) use bootstrap::prepare;
pub use issue::{IdentityService, IssuedAgent};
#[derive(Debug, thiserror::Error)]
#[error("identity operation failed or is no longer authorized")]
pub struct IdentityError;
pub(crate) fn serial_text(serial: &[u8]) -> String {
    use base64ct::Encoding;
    base64ct::Base64UrlUnpadded::encode_string(serial)
}
pub(crate) fn serial_bytes(value: &str) -> Result<Vec<u8>, IdentityError> {
    use base64ct::Encoding;
    if value.len() > 24 {
        return Err(IdentityError);
    }
    let bytes = base64ct::Base64UrlUnpadded::decode_vec(value).map_err(|_| IdentityError)?;
    if bytes.is_empty() || bytes.len() > 17 {
        return Err(IdentityError);
    }
    Ok(bytes)
}
mod client;
pub(crate) use client::configuration as client_configuration;
mod pending;
mod renew;
mod repository;
pub(crate) use client::pending_configuration;
pub use renew::RenewError;
mod client_material;
mod client_renew;
pub(crate) use client_renew::ensure as refresh_client_if_expiring;
