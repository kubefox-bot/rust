use hmail_cleaner_protocol::AgentId;
#[derive(Debug, thiserror::Error)]
#[error("invalid agent HTTPS endpoint")]
pub struct EndpointError;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AgentEndpoint(String);
impl TryFrom<String> for AgentEndpoint {
    type Error = EndpointError;
    fn try_from(input: String) -> Result<Self, Self::Error> {
        if input.len() > 2048 {
            return Err(EndpointError);
        }
        let url = url::Url::parse(&input).map_err(|_| EndpointError)?;
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.host_str().is_none()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(EndpointError);
        }
        Ok(Self(url.origin().ascii_serialization()))
    }
}
impl From<AgentEndpoint> for String {
    fn from(endpoint: AgentEndpoint) -> Self {
        endpoint.0
    }
}
impl AgentEndpoint {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn host(&self) -> Result<String, EndpointError> {
        Ok(url::Url::parse(&self.0)
            .map_err(|_| EndpointError)?
            .host_str()
            .ok_or(EndpointError)?
            .trim_start_matches('[')
            .trim_end_matches(']')
            .into())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCredentials {
    pub agent_id: AgentId,
    pub center_id: uuid::Uuid,
    pub endpoint: AgentEndpoint,
}
