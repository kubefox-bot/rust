use super::AgentCredentials;
use crate::{auth::SessionLease, storage::CenterStore};
use hmail_cleaner_adapters::mtls::ConnectionTrust;
use hmail_cleaner_protocol::{
    AgentHealth, MailProfile, ProfileValidationRequest, ProfileValidationResponse, ResolveRequest,
    ResolvedCutoff, ServiceStateRequest, ServiceStateResponse, PROTOCOL_VERSION,
};
use reqwest::{Method, RequestBuilder};
use serde::de::DeserializeOwned;
use std::time::Duration;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CONTROL_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("agent transport unavailable")]
    Unavailable,
    #[error("agent response violates the protocol")]
    Protocol,
    #[error("agent request rejected")]
    Rejected,
    #[error("agent resource not found")]
    NotFound,
    #[error("agent resource conflict")]
    Conflict,
    #[error("session revoked")]
    Revoked,
}
pub struct AgentClient {
    client: reqwest::Client,
    pub(super) credentials: AgentCredentials,
    pub(super) lease: SessionLease,
    pub(super) trust: ConnectionTrust,
}
impl AgentClient {
    pub async fn new(
        store: &CenterStore,
        credentials: AgentCredentials,
        lease: &SessionLease,
    ) -> Result<Self, AgentError> {
        let trust = store.revoked.capture();
        let tls = crate::identity::client_configuration(store, &credentials, lease)
            .await
            .map_err(|_| {
                if lease.is_live() {
                    AgentError::Unavailable
                } else {
                    AgentError::Revoked
                }
            })?;
        Self::configured(tls, credentials, lease, trust)
    }
    fn configured(
        tls: rustls::ClientConfig,
        credentials: AgentCredentials,
        lease: &SessionLease,
        trust: ConnectionTrust,
    ) -> Result<Self, AgentError> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .http1_only()
            .pool_max_idle_per_host(0)
            .connect_timeout(CONNECT_TIMEOUT)
            .tls_backend_preconfigured(tls)
            .build()
            .map_err(|_| AgentError::Unavailable)?;
        Ok(Self {
            client,
            credentials,
            lease: lease.clone(),
            trust,
        })
    }
    pub async fn health(&self) -> Result<AgentHealth, AgentError> {
        let health: AgentHealth = self
            .control(self.request(Method::GET, "/agent/v1/health"))
            .await?;
        if health.agent_id != self.credentials.agent_id
            || health.protocol_min > PROTOCOL_VERSION
            || health.protocol_max < PROTOCOL_VERSION
            || health.version.len() > 128
            || health.version.is_empty()
            || health.stream_epoch.is_nil()
            || health.revision.get() == 0
        {
            return Err(AgentError::Protocol);
        }
        Ok(health)
    }
    pub async fn resolve(&self, input: &ResolveRequest) -> Result<ResolvedCutoff, AgentError> {
        if input.agent_id != self.credentials.agent_id || input.protocol_version != PROTOCOL_VERSION
        {
            return Err(AgentError::Protocol);
        }
        let cutoff: ResolvedCutoff = self
            .control(self.request(Method::POST, "/agent/v1/resolve").json(input))
            .await?;
        cutoff.validate().map_err(|_| AgentError::Protocol)
    }
    pub async fn validate_profile(
        &self,
        mail: &MailProfile,
    ) -> Result<ProfileValidationResponse, AgentError> {
        self.health().await?;
        let input = ProfileValidationRequest {
            protocol_version: PROTOCOL_VERSION,
            agent_id: self.credentials.agent_id,
            mail_profile: mail.clone(),
        };
        let input = input.validate().map_err(|_| AgentError::Protocol)?;
        self.control(
            self.request(Method::POST, "/agent/v1/profile/validate")
                .json(input.request()),
        )
        .await
    }
    pub async fn service_state(
        &self,
        service_name: &str,
    ) -> Result<ServiceStateResponse, AgentError> {
        let input = ServiceStateRequest {
            protocol_version: PROTOCOL_VERSION,
            agent_id: self.credentials.agent_id,
            service_name: service_name.into(),
        };
        input.validate().map_err(|_| AgentError::Protocol)?;
        self.control(
            self.request(Method::POST, "/agent/v1/service/state")
                .json(&input),
        )
        .await
    }
    pub(crate) fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.client.request(
            method,
            format!("{}{path}", self.credentials.endpoint.as_str()),
        )
    }
    pub(crate) async fn control<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
    ) -> Result<T, AgentError> {
        let mut revoked = self.lease.revocation();
        if !self.lease.is_live() {
            return Err(AgentError::Revoked);
        }
        let operation = async {
            let mut response = request
                .timeout(CONTROL_TIMEOUT)
                .send()
                .await
                .map_err(|_| AgentError::Unavailable)?;
            match response.status().as_u16() {
                200..=299 => (),
                404 => return Err(AgentError::NotFound),
                409 => return Err(AgentError::Conflict),
                400..=499 => return Err(AgentError::Rejected),
                _ => return Err(AgentError::Unavailable),
            }
            if response
                .content_length()
                .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
            {
                return Err(AgentError::Protocol);
            }
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| AgentError::Unavailable)?
            {
                if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(body.len()) {
                    return Err(AgentError::Protocol);
                }
                body.extend_from_slice(&chunk);
            }
            serde_json::from_slice(&body).map_err(|_| AgentError::Protocol)
        };
        tokio::select! {biased;_ = revoked.changed()=>Err(AgentError::Revoked),_ = self.trust.cancelled()=>Err(AgentError::Unavailable),result=operation=>result}
    }
}
impl AgentClient {
    pub(crate) async fn pending(
        store: &CenterStore,
        credentials: AgentCredentials,
        lease: &SessionLease,
    ) -> Result<Self, AgentError> {
        let trust = store.revoked.capture();
        let tls = crate::identity::pending_configuration(store, &credentials, lease)
            .await
            .map_err(|_| {
                if lease.is_live() {
                    AgentError::Unavailable
                } else {
                    AgentError::Revoked
                }
            })?;
        Self::configured(tls, credentials, lease, trust)
    }
    pub(crate) async fn csr(&self) -> Result<hmail_cleaner_protocol::AgentCsr, AgentError> {
        let input = hmail_cleaner_protocol::AgentIdentityRequest {
            protocol_version: PROTOCOL_VERSION,
            agent_id: self.credentials.agent_id,
        };
        let csr: hmail_cleaner_protocol::AgentCsr = self
            .control(
                self.request(Method::POST, "/agent/v1/identity/csr")
                    .json(&input),
            )
            .await?;
        if csr.agent_id != self.credentials.agent_id || csr.csr_pem.len() > 16384 {
            return Err(AgentError::Protocol);
        }
        Ok(csr)
    }
    pub(crate) async fn install_identity(
        &self,
        input: &hmail_cleaner_protocol::AgentIdentityInstall,
    ) -> Result<hmail_cleaner_protocol::IdentityReceipt, AgentError> {
        if input.agent_id != self.credentials.agent_id || input.protocol_version != PROTOCOL_VERSION
        {
            return Err(AgentError::Protocol);
        }
        let receipt: hmail_cleaner_protocol::IdentityReceipt = self
            .control(self.request(Method::PUT, "/agent/v1/identity").json(input))
            .await?;
        if receipt.agent_id != self.credentials.agent_id
            || receipt.serial.is_empty()
            || receipt.serial.len() > 17
        {
            return Err(AgentError::Protocol);
        }
        Ok(receipt)
    }
}
