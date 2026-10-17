use crate::vault::SecretText;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CaSecret {
    pub certificate_pem: String,
    pub private_key_pem: SecretText,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClientSecret {
    pub certificate_pem: String,
    pub private_key_pem: SecretText,
    pub ca_pem: String,
}
