use super::{JobService, SubmitError};
use crate::{
    agents::{stream::AgentEvent, AgentClient},
    auth::SessionLease,
};
use hmail_cleaner_protocol::JobId;
impl JobService {
    pub(super) async fn watch(&self, id: JobId, lease: &SessionLease) -> Result<(), SubmitError> {
        let view = self.get(id, lease).await?;
        if !view.slot_held {
            return Ok(());
        }
        let selected = self.selected(&view, lease).await?;
        let credentials = selected.credentials;
        drop(selected.mail_profile);
        let client = AgentClient::new(&self.store, credentials, lease)
            .await
            .map_err(|_| SubmitError::Protocol)?;
        let mut stream = client.events(id).await.map_err(|_| SubmitError::Protocol)?;
        loop {
            let event = stream.next().await.map_err(|_| SubmitError::Protocol)?;
            let AgentEvent::Snapshot(snapshot) = event else {
                return Ok(());
            };
            let terminal = !snapshot.phase.is_active();
            let _permit = self
                .operation
                .clone()
                .try_lock_owned()
                .map_err(|_| SubmitError::Busy)?;
            self.accept_snapshot(id, *snapshot, &client, lease).await?;
            if terminal || !self.get(id, lease).await?.slot_held {
                return Ok(());
            }
        }
    }
}
