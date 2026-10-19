use super::{
    remote::{Bootstrap, BootstrapIdentity, Connector, Remote},
    EnrollmentError, EnrollmentService,
};
use crate::{auth::SessionLease, identity::IdentityService, profiles::ProfileService};
use hmail_cleaner_protocol::browser::enrollment::EnrollmentView;
impl<C: Connector> EnrollmentService<C> {
    pub(super) async fn recover_identity(
        &self,
        remote: &impl Remote,
        bootstrap: &Bootstrap,
        view: &EnrollmentView,
        lease: &SessionLease,
    ) -> Result<(), EnrollmentError> {
        remote.prepare_recovery(bootstrap).await?;
        super::validation::authorize(lease)?;
        let csr = remote.enroll(bootstrap).await?;
        if csr.agent_id != view.agent_id {
            return Err(EnrollmentError::Remote);
        }
        let credentials = ProfileService::new(self.store.clone())
            .credentials_for_server(view.server_id, lease)
            .await?;
        let host = credentials
            .endpoint
            .host()
            .map_err(|_| EnrollmentError::Invalid)?;
        let port = url::Url::parse(credentials.endpoint.as_str())
            .map_err(|_| EnrollmentError::Invalid)?
            .port_or_known_default()
            .ok_or(EnrollmentError::Invalid)?;
        let issued = IdentityService::new(self.store.clone())
            .recover_agent(view.agent_id, &host, &csr.csr_pem, lease)
            .await
            .map_err(|_| EnrollmentError::Remote)?;
        super::validation::authorize(lease)?;
        remote
            .recover_identity(&BootstrapIdentity {
                bootstrap,
                certificate_pem: &issued.certificate_pem,
                ca_pem: &issued.ca_pem,
                listen_port: port,
            })
            .await
    }
}
