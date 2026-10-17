use serde::{Deserialize, Serialize};
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChangePasswordInput {
    pub current_password: zeroize::Zeroizing<String>,
    pub new_password: zeroize::Zeroizing<String>,
    pub confirm_password: zeroize::Zeroizing<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareRecoveryInput {
    pub current_password: zeroize::Zeroizing<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryCandidate {
    pub candidate_id: uuid::Uuid,
    pub recovery_key: zeroize::Zeroizing<String>,
    pub expires_in_seconds: u16,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmRecoveryInput {
    pub candidate_id: uuid::Uuid,
    pub recovery_key: zeroize::Zeroizing<String>,
}
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
