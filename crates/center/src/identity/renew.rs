use super::{pending, repository, IdentityError, IdentityService, IssuedAgent};
use crate::{
    agents::{AgentClient, AgentCredentials, AgentError},
    auth::SessionLease,
};
use hmail_cleaner_protocol::{AgentIdentityInstall, PROTOCOL_VERSION};
#[derive(Debug, thiserror::Error)]
pub enum RenewError {
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Agent(#[from] AgentError),
}
impl IdentityService {
    pub async fn renew_agent(
        &self,
        credentials: AgentCredentials,
        lease: &SessionLease,
    ) -> Result<IssuedAgent, RenewError> {
        let agent = credentials.agent_id;
        let current = pending::current(&self.store, agent).await?;
        super::client_renew::ensure(&self.store, lease).await?;
        let old_client = AgentClient::new(&self.store, credentials.clone(), lease).await?;
        let pending = match pending::load(&self.store, agent).await? {
            Some(pending) => pending,
            None => {
                let csr = old_client.csr().await?;
                let issued = self
                    .sign_agent(
                        agent,
                        &credentials.endpoint.host().map_err(|_| IdentityError)?,
                        &csr.csr_pem,
                        lease,
                    )
                    .await?;
                pending::prepare(
                    self.store.clone(),
                    lease.clone(),
                    agent,
                    pending::Pending {
                        expected_serial: current.clone(),
                        issued,
                    },
                )
                .await?
            }
        };
        if pending.expected_serial != current {
            return Err(IdentityError.into());
        }
        let candidate = AgentClient::pending(&self.store, credentials.clone(), lease).await?;
        if candidate.health().await.is_err() {
            let input = AgentIdentityInstall {
                protocol_version: PROTOCOL_VERSION,
                agent_id: agent,
                server_name: credentials.endpoint.host().map_err(|_| IdentityError)?,
                certificate_pem: pending.issued.certificate_pem.clone(),
            };
            let delivered = old_client.install_identity(&input).await;
            if candidate.health().await.is_err() {
                return Err(delivered.err().unwrap_or(AgentError::Unavailable).into());
            }
        }
        Ok(repository::activate(
            self.store.clone(),
            lease.clone(),
            agent,
            pending.issued,
            repository::Activation::Renewal {
                expected_serial: pending.expected_serial,
            },
        )
        .await?)
    }
}
