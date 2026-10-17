use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub login: String,
    pub password: zeroize::Zeroizing<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionView {
    pub authenticated: bool,
    pub generation: Option<crate::Counter>,
}
