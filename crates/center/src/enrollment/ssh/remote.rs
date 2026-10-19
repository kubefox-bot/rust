use super::{command::Step, NativeSsh, VerifiedSsh};
use crate::{
    auth::SessionLease,
    enrollment::{
        remote::{Bootstrap, BootstrapIdentity, Connector, Remote},
        AgentPackage, EnrollmentError, SshCredentials, SshTarget,
    },
};
use hmail_cleaner_protocol::{browser::enrollment::LegacyService, AgentCsr};
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    ok: bool,
}
impl Ready {
    fn check(self) -> Result<(), EnrollmentError> {
        if self.ok {
            Ok(())
        } else {
            Err(EnrollmentError::Remote)
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stage {
    path: String,
}
#[derive(Serialize)]
struct Installation<'a> {
    bootstrap: &'a Bootstrap,
    legacy: Option<&'a LegacyService>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    operation_id: Option<uuid::Uuid>,
    executable: String,
    sha256: String,
    previous_sha256: Option<String>,
    started: bool,
}
impl Connector for NativeSsh {
    type Connection = VerifiedSsh;
    async fn connect(
        &self,
        target: SshTarget,
        fingerprint: String,
        credentials: SshCredentials,
        lease: &SessionLease,
    ) -> Result<VerifiedSsh, EnrollmentError> {
        NativeSsh::connect(target, fingerprint, credentials, lease).await
    }
}
impl Remote for VerifiedSsh {
    async fn prepare_recovery(&self, input: &Bootstrap) -> Result<(), EnrollmentError> {
        self.execute::<Ready>(Step::PrepareRecovery, input)
            .await?
            .check()
    }
    async fn recover_identity(&self, input: &BootstrapIdentity<'_>) -> Result<(), EnrollmentError> {
        self.execute::<Ready>(Step::RecoverIdentity, input)
            .await?
            .check()
    }
    async fn cleanup(&self, input: &Bootstrap) -> Result<(), EnrollmentError> {
        self.execute::<Ready>(Step::Cleanup, input).await?.check()
    }
    async fn preflight(&self) -> Result<(), EnrollmentError> {
        self.execute::<Ready>(Step::Preflight, &serde_json::json!({}))
            .await?
            .check()
    }
    async fn prepare(&self, input: &Bootstrap) -> Result<String, EnrollmentError> {
        Ok(self.execute::<Stage>(Step::Prepare, input).await?.path)
    }
    async fn upload(&self, path: &str, package: &mut AgentPackage) -> Result<(), EnrollmentError> {
        self.transfer(path, package).await
    }
    async fn enroll(&self, input: &Bootstrap) -> Result<AgentCsr, EnrollmentError> {
        let csr = self.execute::<AgentCsr>(Step::Enroll, input).await?;
        if csr.agent_id != input.agent_id || csr.csr_pem.len() > 16384 {
            return Err(EnrollmentError::Remote);
        }
        Ok(csr)
    }
    async fn configure(&self, input: &BootstrapIdentity<'_>) -> Result<(), EnrollmentError> {
        self.execute::<Ready>(Step::Configure, input).await?.check()
    }
    async fn install(
        &self,
        input: &Bootstrap,
        legacy: Option<&LegacyService>,
    ) -> Result<(), EnrollmentError> {
        let receipt = self
            .execute::<Receipt>(
                Step::Install,
                &Installation {
                    bootstrap: input,
                    legacy,
                },
            )
            .await?;
        if receipt.operation_id != Some(input.operation_id)
            || receipt.sha256 != input.sha256
            || !receipt.started
            || !receipt
                .executable
                .ends_with("\\hMailCleanerAgent\\hmail-cleaner-agent.exe")
            || receipt
                .previous_sha256
                .as_ref()
                .is_some_and(|value| value.len() != 64)
        {
            return Err(EnrollmentError::Remote);
        }
        Ok(())
    }
    async fn close(self) -> Result<(), EnrollmentError> {
        VerifiedSsh::close(self).await
    }
}
