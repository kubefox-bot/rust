use super::{
    remote::{BootstrapIdentity, Connector, Remote},
    repository::Kind,
    EnrollmentError, EnrollmentService,
};
use crate::{
    auth::SessionLease,
    identity::IdentityService,
    profiles::{ProfileService, ServerRegistration},
};
use hmail_cleaner_protocol::browser::enrollment::{
    EnrollmentPhase, EnrollmentView, InstallRequest,
};
impl<C: Connector> EnrollmentService<C> {
    pub async fn install(
        &self,
        lease: &SessionLease,
        input: InstallRequest,
    ) -> Result<EnrollmentView, EnrollmentError> {
        super::validation::authorize(lease)?;
        let (host, listen_port) = super::validation::install(&input)?;
        let package = self.catalog.open(&input.package_version).await?;
        let view = super::validation::view(&input, &package);
        let service = self.clone();
        let lease = lease.clone();
        tokio::spawn(async move {
            service.reserve(&view, &lease, Kind::Install).await?;
            let response = view.clone();
            tokio::spawn(async move {
                let mut view = view;
                let result = service
                    .install_owned(&lease, input, package, &mut view, host, listen_port)
                    .await;
                if result.is_err() {
                    service.failed(&mut view).await;
                }
            });
            Ok(response)
        })
        .await
        .map_err(|_| EnrollmentError::Storage)?
    }
    async fn install_owned(
        &self,
        lease: &SessionLease,
        input: InstallRequest,
        mut package: super::AgentPackage,
        view: &mut EnrollmentView,
        host: String,
        listen_port: u16,
    ) -> Result<(), EnrollmentError> {
        let bootstrap = super::validation::bootstrap(&self.store, view, lease).await?;
        let remote = self
            .connector
            .connect(input.target, input.fingerprint, input.credentials, lease)
            .await?;
        let result = async {
            remote.preflight().await?;
            self.record(view, EnrollmentPhase::Transferring).await?;
            let stage = remote.prepare(&bootstrap).await?;
            super::validation::authorize(lease)?;
            remote.upload(&stage, &mut package).await?;
            super::validation::authorize(lease)?;
            let csr = remote.enroll(&bootstrap).await?;
            if csr.agent_id != input.agent_id {
                return Err(EnrollmentError::Remote);
            }
            let issued = IdentityService::new(self.store.clone())
                .issue_agent(input.agent_id, &host, &csr.csr_pem, lease)
                .await
                .map_err(|_| EnrollmentError::Remote)?;
            ProfileService::new(self.store.clone())
                .register(
                    ServerRegistration {
                        server_id: input.server_id,
                        profile_id: input.profile_id,
                        agent_id: input.agent_id,
                        name: input.name,
                        endpoint: input.endpoint,
                    },
                    lease,
                )
                .await?;
            self.record(view, EnrollmentPhase::IdentityIssued).await?;
            remote
                .configure(&BootstrapIdentity {
                    bootstrap: &bootstrap,
                    certificate_pem: &issued.certificate_pem,
                    ca_pem: &issued.ca_pem,
                    listen_port,
                })
                .await?;
            self.record(view, EnrollmentPhase::Installing).await?;
            super::validation::authorize(lease)?;
            remote.install(&bootstrap, input.legacy.as_ref()).await?;
            self.record(view, EnrollmentPhase::Verifying).await?;
            self.verify(view, lease).await?;
            if remote.cleanup(&bootstrap).await.is_err() {
                eprintln!("Verified installation left its staged package for inspection");
            }
            Ok(())
        }
        .await;
        if remote.close().await.is_err() {
            eprintln!("SSH enrollment close failed");
        }
        result
    }
}
