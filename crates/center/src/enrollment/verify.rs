use super::{remote::Connector, EnrollmentError, EnrollmentService};
use crate::{agents::AgentClient, auth::SessionLease, profiles::ProfileService};
use hmail_cleaner_protocol::browser::enrollment::{EnrollmentPhase, EnrollmentView};
const VERIFY_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);
const VERIFY_RETRY: std::time::Duration = std::time::Duration::from_secs(1);
impl<C: Connector> EnrollmentService<C> {
    pub(super) async fn verify(
        &self,
        view: &mut EnrollmentView,
        lease: &SessionLease,
    ) -> Result<(), EnrollmentError> {
        crate::identity::refresh_client_if_expiring(&self.store, lease)
            .await
            .map_err(|_| EnrollmentError::Remote)?;
        let credentials = ProfileService::new(self.store.clone())
            .credentials_for_server(view.server_id, lease)
            .await?;
        if credentials.agent_id != view.agent_id {
            return Err(EnrollmentError::Remote);
        }
        let client = AgentClient::new(&self.store, credentials, lease)
            .await
            .map_err(|_| EnrollmentError::Remote)?;
        let proof = async {
            loop {
                super::validation::authorize(lease)?;
                if verify_proof(&client, view).await.is_ok() {
                    break Ok::<(), EnrollmentError>(());
                }
                let mut revoked = lease.revocation();
                tokio::select! {biased;_=revoked.changed()=>return Err(EnrollmentError::Unauthorized),_=tokio::time::sleep(VERIFY_RETRY)=>()}
            }
        };
        tokio::time::timeout(VERIFY_DEADLINE, proof)
            .await
            .map_err(|_| EnrollmentError::Remote)??;
        self.record(view, EnrollmentPhase::Ready).await
    }
}
async fn verify_proof(client: &AgentClient, view: &EnrollmentView) -> Result<(), EnrollmentError> {
    let health = client
        .health()
        .await
        .map_err(|_| EnrollmentError::Transport)?;
    if !health.healthy || !health.idle || health.version != view.package_version {
        return Err(EnrollmentError::Remote);
    }
    let receipt = client
        .installation()
        .await
        .map_err(|_| EnrollmentError::Remote)?;
    if receipt.operation_id != view.id || receipt.sha256 != view.expected_sha256 {
        return Err(EnrollmentError::Remote);
    }
    Ok(())
}
