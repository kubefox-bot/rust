use super::{JobService, SubmitError};
use crate::{auth::SessionLease, profiles::ProfileService};
use hmail_cleaner_protocol::{browser::jobs::CleanupContext, ServerId};
impl JobService {
    pub async fn cleanup_context(
        &self,
        server_id: ServerId,
        lease: &SessionLease,
    ) -> Result<CleanupContext, SubmitError> {
        let profiles = ProfileService::new(self.store.clone());
        let server = profiles
            .servers(lease)
            .await?
            .into_iter()
            .find(|server| server.id == server_id)
            .ok_or(SubmitError::NotFound)?;
        let profile = profiles.profile_for_server(server_id, lease).await?;
        let latest = self
            .history(server_id, None, 1, lease)
            .await?
            .jobs
            .into_iter()
            .next();
        let busy: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM job_slot WHERE job_id IS NOT NULL) OR EXISTS(SELECT 1 FROM provisioning_slot WHERE enrollment_id IS NOT NULL)").fetch_one(&self.store.pool).await?;
        if !lease.is_live() {
            return Err(SubmitError::Unauthorized);
        }
        Ok(CleanupContext {
            server,
            profile_version: profile.version,
            configured: profile.configured,
            latest,
            busy,
        })
    }
}
