use serde::{Deserialize, Serialize};
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangePasswordInput {
    #[schemars(with = "String")]
    pub current_password: zeroize::Zeroizing<String>,
    #[schemars(with = "String")]
    pub new_password: zeroize::Zeroizing<String>,
    #[schemars(with = "String")]
    pub confirm_password: zeroize::Zeroizing<String>,
}
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrepareRecoveryInput {
    #[schemars(with = "String")]
    pub current_password: zeroize::Zeroizing<String>,
}
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoveryCandidate {
    pub candidate_id: uuid::Uuid,
    #[schemars(with = "String")]
    pub recovery_key: zeroize::Zeroizing<String>,
    pub expires_in_seconds: u16,
}
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfirmRecoveryInput {
    pub candidate_id: uuid::Uuid,
    #[schemars(with = "String")]
    pub recovery_key: zeroize::Zeroizing<String>,
}
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub login: String,
    #[schemars(with = "String")]
    pub password: zeroize::Zeroizing<String>,
}
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionView {
    pub authenticated: bool,
    pub generation: Option<crate::Counter>,
}
