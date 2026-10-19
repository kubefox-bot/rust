use crate::{agents::AgentCredentials, storage::CenterStore};
use hmail_cleaner_protocol::{AgentId, MailProfile, ProfileId, ServerId};
use serde::{Deserialize, Serialize};
#[derive(Clone)]
pub struct ProfileService {
    pub(super) store: CenterStore,
}
impl ProfileService {
    pub fn new(store: CenterStore) -> Self {
        Self { store }
    }
}
pub struct ServerRegistration {
    pub server_id: ServerId,
    pub profile_id: ProfileId,
    pub agent_id: AgentId,
    pub name: String,
    pub endpoint: String,
}
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("invalid profile")]
    Invalid,
    #[error("profile version conflict")]
    Conflict,
    #[error("profile not found or not configured")]
    NotFound,
    #[error("profile could not be read or saved")]
    Storage,
    #[error("session expired")]
    Unauthorized,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TechnicalProfile {
    pub server_id: ServerId,
    pub profile_id: ProfileId,
    pub version: u64,
    pub credentials: AgentCredentials,
    pub mail_profile: Option<MailProfile>,
}
pub struct SelectedProfile {
    pub server_id: ServerId,
    pub profile_id: ProfileId,
    pub version: u64,
    pub credentials: AgentCredentials,
    pub mail_profile: MailProfile,
}
pub(super) struct StoredProfile {
    pub technical: TechnicalProfile,
    pub material: crate::vault::AccountMaterial,
}
impl From<crate::agents::EndpointError> for ProfileError {
    fn from(_: crate::agents::EndpointError) -> Self {
        Self::Invalid
    }
}
