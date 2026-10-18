use super::{JobService, SubmitError};
use crate::{agents::AgentClient, auth::SessionLease};
use hmail_cleaner_protocol::{browser::jobs::JobView, JobId};
impl JobService {
    pub async fn cancel(&self, id: JobId, lease: &SessionLease) -> Result<JobView, SubmitError> {
        let maintenance = self
            .store
            .maintenance()
            .continuation()
            .map_err(|_| SubmitError::Busy)?;
        let permit = self
            .operation
            .clone()
            .try_lock_owned()
            .map_err(|_| SubmitError::Busy)?;
        let service = self.clone();
        let lease = lease.clone();
        tokio::spawn(async move {
            let _maintenance = maintenance;
            let _permit = permit;
            let view = service.get(id, &lease).await?;
            if !view.slot_held {
                return Ok(view);
            }
            let selected = service.selected(&view, &lease).await?;
            let credentials = selected.credentials;
            drop(selected.mail_profile);
            if let Ok(client) = AgentClient::new(&service.store, credentials, &lease).await {
                if let Ok(snapshot) = client.cancel_job(id).await {
                    service
                        .accept_snapshot(id, snapshot, &client, &lease)
                        .await?;
                }
            }
            service.load(id).await
        })
        .await
        .map_err(|_| SubmitError::Storage)?
    }
}
