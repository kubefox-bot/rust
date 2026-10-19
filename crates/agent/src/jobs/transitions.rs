use super::{JobRepository, JobStoreError, JobUpdate};
use hmail_cleaner_protocol::{JobId, JobSnapshot};
impl JobRepository {
    pub async fn transition(
        &self,
        id: JobId,
        update: JobUpdate,
    ) -> Result<JobSnapshot, JobStoreError> {
        let mut transaction = self.writer().await?;
        let row = sqlx::query("SELECT job_id, phase, snapshot FROM jobs WHERE job_id = ?")
            .bind(id.to_string())
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(JobStoreError::NotFound)?;
        let mut snapshot = self.decode(&row, id)?;
        if !snapshot.phase.permits(update.phase) {
            return Err(JobStoreError::InvalidTransition);
        }
        snapshot.phase = update.phase;
        snapshot.stage = update.stage;
        snapshot.summary = update.summary;
        snapshot.mail_state = update.mail_state;
        snapshot.error = update.error;
        self.bump(&mut snapshot)?;
        self.save(&mut transaction, &snapshot).await?;
        if !snapshot.phase.is_active() {
            super::retention::prune(&mut transaction).await?;
        }
        transaction.commit().await?;
        Ok(snapshot)
    }
}
