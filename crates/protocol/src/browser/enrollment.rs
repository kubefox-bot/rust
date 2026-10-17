use crate::{AgentId, ProfileId, ServerId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SshTarget {
    pub host: String,
    pub port: u16,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostKeyChallenge {
    pub target: SshTarget,
    pub fingerprint: String,
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum SshCredentials {
    Password {
        username: String,
        #[schemars(with = "String")]
        password: Zeroizing<String>,
    },
    Key {
        username: String,
        #[schemars(with = "String")]
        private_key: Zeroizing<String>,
        #[schemars(with = "Option<String>")]
        passphrase: Option<Zeroizing<String>>,
    },
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstallRequest {
    pub operation_id: Uuid,
    pub agent_id: AgentId,
    pub server_id: ServerId,
    pub profile_id: ProfileId,
    pub name: String,
    pub endpoint: String,
    pub target: SshTarget,
    pub fingerprint: String,
    pub credentials: SshCredentials,
    pub package_version: String,
    pub legacy: Option<LegacyService>,
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateRequest {
    #[serde(default)]
    pub mode: UpdateMode,
    pub operation_id: Uuid,
    pub target: SshTarget,
    pub fingerprint: String,
    pub credentials: SshCredentials,
    pub package_version: String,
}
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    #[default]
    Binary,
    RecoverIdentity,
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LegacyService {
    pub executable: String,
    pub env_file: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnrollmentPhase {
    Preparing,
    Transferring,
    IdentityIssued,
    Installing,
    Verifying,
    Ready,
    Failed,
    Unknown,
}
impl EnrollmentPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Transferring => "transferring",
            Self::IdentityIssued => "identity_issued",
            Self::Installing => "installing",
            Self::Verifying => "verifying",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentView {
    pub id: Uuid,
    pub agent_id: AgentId,
    pub server_id: ServerId,
    pub profile_id: ProfileId,
    pub phase: EnrollmentPhase,
    pub fingerprint: String,
    pub package_version: String,
    pub expected_sha256: String,
    pub slot_held: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentPackageView {
    pub build: String,
    pub sha256: String,
}
