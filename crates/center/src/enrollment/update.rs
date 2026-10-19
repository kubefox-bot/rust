use super::{
    remote::{Connector, Remote},
    repository::Kind,
    EnrollmentError, EnrollmentService,
};
use crate::{agents::AgentClient, auth::SessionLease, profiles::ProfileService};
use hmail_cleaner_protocol::{
    browser::enrollment::{EnrollmentPhase, EnrollmentView, UpdateMode, UpdateRequest},
    AgentId,
};
impl<C: Connector> EnrollmentService<C> {
    pub async fn update(
        &self,
        lease: &SessionLease,
        id: AgentId,
        input: UpdateRequest,
    ) -> Result<EnrollmentView, EnrollmentError> {
        let maintenance = self
            .store
            .maintenance()
            .admit()
            .map_err(|_| EnrollmentError::Busy)?;
        super::validation::authorize(lease)?;
        if input.operation_id.is_nil() {
            return Err(EnrollmentError::Invalid);
        }
        let server = self.server_for_agent(id).await?;
        let profiles = ProfileService::new(self.store.clone());
        let profile = profiles.active_profile(server).await?;
        let credentials = profiles.credentials_for_server(server, lease).await?;
        if credentials
            .endpoint
            .host()
            .map_err(|_| EnrollmentError::Invalid)?
            != input.target.host
        {
            return Err(EnrollmentError::Invalid);
        }
        crate::identity::refresh_client_if_expiring(&self.store, lease)
            .await
            .map_err(|_| EnrollmentError::Remote)?;
        match input.mode {
            UpdateMode::Binary => {
                let client = AgentClient::new(&self.store, credentials, lease)
                    .await
                    .map_err(|_| EnrollmentError::Remote)?;
                let health = client
                    .health()
                    .await
                    .map_err(|_| EnrollmentError::Transport)?;
                if !health.healthy || !health.idle {
                    return Err(EnrollmentError::Busy);
                }
            }
            UpdateMode::RecoverIdentity => (),
        }
        let package = self.catalog.open(&input.package_version).await?;
        let view = EnrollmentView {
            id: input.operation_id,
            agent_id: id,
            server_id: server,
            profile_id: profile,
            phase: EnrollmentPhase::Preparing,
            fingerprint: input.fingerprint.clone(),
            package_version: package.metadata().build.clone(),
            expected_sha256: package.metadata().sha256.clone(),
            slot_held: true,
        };
        let service = self.clone();
        let lease = lease.clone();
        tokio::spawn(async move {
            service.reserve(&view, &lease, Kind::Update).await?;
            let response = view.clone();
            tokio::spawn(async move {
                let _maintenance = maintenance;
                let mut view = view;
                let result = service
                    .update_owned(&lease, input, package, &mut view)
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
    async fn update_owned(
        &self,
        lease: &SessionLease,
        input: UpdateRequest,
        mut package: super::AgentPackage,
        view: &mut EnrollmentView,
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
            self.record(view, EnrollmentPhase::Installing).await?;
            super::validation::authorize(lease)?;
            match input.mode {
                UpdateMode::Binary => (),
                UpdateMode::RecoverIdentity => {
                    self.recover_identity(&remote, &bootstrap, view, lease)
                        .await?
                }
            }
            super::validation::authorize(lease)?;
            remote.install(&bootstrap, None).await?;
            self.record(view, EnrollmentPhase::Verifying).await?;
            self.verify(view, lease).await?;
            if remote.cleanup(&bootstrap).await.is_err() {
                eprintln!("Verified update left its staged package for inspection");
            }
            Ok(())
        }
        .await;
        if remote.close().await.is_err() {
            eprintln!("SSH update close failed");
        }
        result
    }
}
