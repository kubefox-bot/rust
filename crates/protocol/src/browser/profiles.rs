use crate::{AgentId, Counter, DatabaseTls, ProfileId, ServerId};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum PasswordPatch {
    Keep,
    Replace(#[schemars(with = "String")] zeroize::Zeroizing<String>),
}
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfilePatch {
    pub mysql_host: String,
    pub mysql_port: u16,
    pub mysql_database: String,
    pub mysql_username: String,
    pub password: PasswordPatch,
    pub mysql_tls: DatabaseTls,
    pub data_dir: String,
    pub hmail_ini: String,
    pub service_name: String,
}
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditableProfile {
    pub id: ProfileId,
    pub version: Counter,
    pub agent_id: AgentId,
    pub configured: bool,
    pub mysql_host: String,
    pub mysql_port: u16,
    pub mysql_database: String,
    pub mysql_username: String,
    pub password_set: bool,
    pub mysql_tls: DatabaseTls,
    pub data_dir: String,
    pub hmail_ini: String,
    pub service_name: String,
}
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    pub id: ServerId,
    pub name: String,
    pub agent_id: AgentId,
    pub profile_id: ProfileId,
}
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveProfileRequest {
    pub expected_version: Counter,
    pub profile: ProfilePatch,
}
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileVersion {
    pub id: ProfileId,
    pub version: Counter,
}
