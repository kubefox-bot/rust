use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthView {
    pub agent_id: crate::AgentId,
    pub online: bool,
    pub stale: bool,
    pub healthy: Option<bool>,
    pub version: Option<String>,
    pub observed_at: Option<chrono::DateTime<chrono::Utc>>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentView {
    pub agent_id: crate::AgentId,
    pub server_id: crate::ServerId,
    pub name: String,
    pub revoked: bool,
    pub health: HealthView,
    pub certificate_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub certificate_expiring: bool,
    pub certificate_expired: bool,
}
