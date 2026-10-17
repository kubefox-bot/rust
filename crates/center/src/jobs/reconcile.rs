use super::{JobService, SubmitError};
use crate::{
    agents::AgentClient,
    auth::SessionLease,
    profiles::{ProfileService, SelectedProfile},
};
use hmail_cleaner_protocol::{browser::jobs::JobView, JobId, JobSnapshot};
const MAX_SERVICE_OBSERVATION_AGE_SECONDS: u64 = 30;
impl JobService {
    pub async fn reconcile(&self, id: JobId, lease: &SessionLease) -> Result<JobView, SubmitError> {
        let permit = self
            .operation
            .clone()
            .try_lock_owned()
            .map_err(|_| SubmitError::Busy)?;
        let service = self.clone();
        let lease = lease.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let view = service.get(id, &lease).await?;
            if !view.slot_held {
                return Ok(view);
            }
            let selected = service.selected(&view, &lease).await?;
            let credentials = selected.credentials;
            drop(selected.mail_profile);
            if let Ok(client) = AgentClient::new(&service.store, credentials, &lease).await {
                // Absence and transport failure both preserve uncertainty and the reservation.
                if let Ok(snapshot) = client.job(id).await {
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
    pub(super) async fn selected(
        &self,
        view: &JobView,
        lease: &SessionLease,
    ) -> Result<SelectedProfile, SubmitError> {
        Ok(ProfileService::new(self.store.clone())
            .for_job(
                view.intent.server_id,
                self.profile_id(view.job_id).await?,
                view.intent.profile_version.get(),
                lease,
            )
            .await?)
    }
    pub(super) async fn accept_snapshot(
        &self,
        id: JobId,
        snapshot: JobSnapshot,
        client: &AgentClient,
        lease: &SessionLease,
    ) -> Result<(), SubmitError> {
        let view = self.load(id).await?;
        let selected = self.selected(&view, lease).await?;
        super::receipt::validate(&view, &snapshot, &selected)?;
        let service_name = selected.mail_profile.service_name.clone();
        drop(selected);
        let release = if snapshot.phase.is_active() {
            false
        } else {
            match (
                client.health().await,
                client.service_state(&service_name).await,
            ) {
                (Ok(health), Ok(state))
                    if health.healthy
                        && health.stream_epoch == snapshot.stream_epoch
                        && (chrono::Utc::now() - state.observed_at)
                            .num_seconds()
                            .unsigned_abs()
                            <= MAX_SERVICE_OBSERVATION_AGE_SECONDS =>
                {
                    super::receipt::safe_to_release(&view, &snapshot, state.mail_state)
                }
                _ => false,
            }
        };
        if view.snapshot.as_ref() == Some(&snapshot) && !release {
            return Ok(());
        }
        let mut tx = self.writer().await?;
        sqlx::query(
            "UPDATE jobs SET phase=?,snapshot=?,revision=revision+1 WHERE id=? AND revision=?",
        )
        .bind(snapshot.phase.as_str())
        .bind(serde_json::to_vec(&snapshot)?)
        .bind(id.to_string())
        .bind(i64::try_from(view.revision.get()).map_err(|_| SubmitError::Storage)?)
        .execute(&mut *tx)
        .await?
        .rows_affected()
        .eq(&1)
        .then_some(())
        .ok_or(SubmitError::Conflict)?;
        if release {
            sqlx::query("UPDATE job_slot SET job_id=NULL WHERE singleton=1 AND job_id=?")
                .bind(id.to_string())
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        self.publish(id).await
    }
}
