use super::{JobRepository, JobStoreError};
use hmail_cleaner_protocol::{ErrorCode, JobId, JobPhase, ObservedMailState, PublicError};
use sqlx::Row;

impl JobRepository {
    /// Called once by the boot owner before accepting work. Never dispatches a job.
    pub async fn recover_interrupted(&self) -> Result<(), JobStoreError> {
        let mut transaction = self.writer().await?;
        let rows = sqlx::query("SELECT job_id, phase, snapshot FROM jobs WHERE phase IN ('accepted','running','cancelling','restoring')")
            .fetch_all(&mut *transaction).await?;
        for row in rows {
            let id: JobId =
                serde_json::from_value(serde_json::Value::String(row.try_get("job_id")?))?;
            let mut snapshot = self.decode(&row, id)?;
            snapshot.phase = JobPhase::Interrupted;
            snapshot.mail_state = ObservedMailState::Unknown;
            snapshot.error = Some(PublicError::new(ErrorCode::Interrupted));
            self.bump(&mut snapshot)?;
            self.save(&mut transaction, &snapshot).await?;
        }
        super::retention::prune(&mut transaction).await?;
        transaction.commit().await?;
        Ok(())
    }
}
