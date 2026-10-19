use crate::{AgentId, Counter, MailProfile, ProtocolError, PROTOCOL_VERSION};
use chrono::NaiveDateTime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RetentionRequest {
    Days { count: NonZeroU32 },
    Months { count: NonZeroU32 },
    Years { count: NonZeroU32 },
    Before { local: NaiveDateTime },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveRequest {
    pub protocol_version: u16,
    pub agent_id: AgentId,
    pub retention: RetentionRequest,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileValidationRequest {
    pub protocol_version: u16,
    pub agent_id: AgentId,
    pub mail_profile: MailProfile,
}
impl ProfileValidationRequest {
    pub fn validate(self) -> Result<ValidatedProfile, ProtocolError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        if !self.mail_profile.is_valid() {
            return Err(ProtocolError::InvalidProfile);
        }
        Ok(ValidatedProfile(self))
    }
}
pub struct ValidatedProfile(ProfileValidationRequest);
impl ValidatedProfile {
    pub fn into_profile(self) -> MailProfile {
        self.0.mail_profile
    }
    pub fn request(&self) -> &ProfileValidationRequest {
        &self.0
    }
    pub fn agent_id(&self) -> AgentId {
        self.0.agent_id
    }
    pub fn profile(&self) -> &MailProfile {
        &self.0.mail_profile
    }
}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileValidationResponse {
    pub mail_state: crate::ObservedMailState,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentHealth {
    pub agent_id: AgentId,
    pub version: String,
    pub protocol_min: u16,
    pub protocol_max: u16,
    pub stream_epoch: uuid::Uuid,
    pub revision: Counter,
    pub healthy: bool,
    #[serde(default)]
    pub idle: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationStatus {
    pub operation_id: uuid::Uuid,
    pub sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceStateRequest {
    pub protocol_version: u16,
    pub agent_id: AgentId,
    pub service_name: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceStateResponse {
    pub mail_state: crate::ObservedMailState,
    pub observed_at: chrono::DateTime<chrono::Utc>,
}
impl ServiceStateRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        if self.service_name.trim().is_empty()
            || self.service_name.len() > crate::limits::MAX_PROFILE_TEXT_BYTES
            || self.service_name.contains('\0')
            || hmail_cleaner_core::maintenance::mail_service::is_reserved_service_name(
                &self.service_name,
            )
        {
            return Err(ProtocolError::InvalidProfile);
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCsr {
    pub agent_id: AgentId,
    pub csr_pem: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentIdentityRequest {
    pub protocol_version: u16,
    pub agent_id: AgentId,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentIdentityInstall {
    pub protocol_version: u16,
    pub agent_id: AgentId,
    pub server_name: String,
    pub certificate_pem: String,
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentityReceipt {
    pub agent_id: AgentId,
    pub serial: Vec<u8>,
}
