use super::{persistence, validation, IdentityBootstrap};
use crate::{
    config::AgentConfig, enrollment::AgentCsr, execution::AgentError, jobs::JobRepository, service,
};
use hmail_cleaner_adapters::mtls::ServerTls;
use hmail_cleaner_protocol::{AgentIdentityInstall, IdentityReceipt};
use std::sync::Arc;
#[derive(Clone)]
pub(crate) struct IdentityManager {
    bootstrap: Arc<IdentityBootstrap>,
    repository: Arc<JobRepository>,
    tls: ServerTls,
}
impl IdentityManager {
    pub fn new(config: &AgentConfig, repository: Arc<JobRepository>) -> Self {
        Self {
            bootstrap: config.identity.clone(),
            repository,
            tls: config.tls.clone(),
        }
    }
    pub fn agent_id(&self) -> hmail_cleaner_protocol::AgentId {
        self.bootstrap.agent_id
    }
    pub fn csr(&self) -> Result<AgentCsr, AgentError> {
        let params = rcgen::CertificateParams::new(Vec::<String>::new())
            .map_err(|_| AgentError::Unavailable)?;
        let request = params
            .serialize_request(self.bootstrap.key.as_ref())
            .map_err(|_| AgentError::Unavailable)?;
        Ok(AgentCsr {
            agent_id: self.bootstrap.agent_id,
            csr_pem: request.pem().map_err(|_| AgentError::Unavailable)?,
        })
    }
    pub async fn install(
        &self,
        input: AgentIdentityInstall,
        on_incomplete: impl FnOnce() + Send + 'static,
    ) -> Result<IdentityReceipt, AgentError> {
        let candidate = validation::prepare(&self.bootstrap, &input)?;
        let lock =
            service::acquire_update_lock(&self.bootstrap.directory).map_err(
                |error| match error {
                    service::InstallError::Busy => AgentError::Busy,
                    _ => AgentError::Unavailable,
                },
            )?;
        let manager = self.clone();
        tokio::spawn(async move {
            let _lock = lock;
            let result = manager.commit(input, candidate).await;
            if matches!(
                &result,
                Err(AgentError::Unavailable | AgentError::Storage(_) | AgentError::Transport(_))
            ) {
                on_incomplete();
            }
            result
        })
        .await
        .map_err(|_| AgentError::Unavailable)?
    }
    async fn commit(
        &self,
        input: AgentIdentityInstall,
        candidate: validation::Candidate,
    ) -> Result<IdentityReceipt, AgentError> {
        if self.repository.in_maintenance().await? {
            return Err(AgentError::Unavailable);
        }
        if persistence::current(&self.bootstrap, &candidate)? {
            return Ok(candidate.receipt);
        }
        if !self.repository.begin_maintenance().await? {
            return Err(AgentError::Busy);
        }
        persistence::persist(&self.bootstrap, &input.certificate_pem)?;
        self.tls.replace(candidate.tls);
        *self
            .bootstrap
            .fingerprint
            .lock()
            .map_err(|_| AgentError::Unavailable)? = candidate.fingerprint;
        self.repository.end_maintenance().await?;
        Ok(candidate.receipt)
    }
}
