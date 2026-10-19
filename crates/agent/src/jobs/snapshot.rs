use super::{JobRepository, JobStoreError};
use chrono::Utc;
use hmail_cleaner_protocol::{JobId, JobPhase, JobSnapshot};
use sqlx::{sqlite::SqliteRow, Row, Sqlite, Transaction};
impl JobRepository {
    pub(super) fn decode(&self, row: &SqliteRow, id: JobId) -> Result<JobSnapshot, JobStoreError> {
        let mut snapshot: JobSnapshot = serde_json::from_str(row.try_get("snapshot")?)?;
        let phase: String = row.try_get("phase")?;
        if snapshot.job_id != id
            || phase != snapshot.phase.as_str()
            || snapshot.revision.get() == 0
            || (snapshot.job.is_none() && snapshot.phase != JobPhase::Cancelled)
        {
            return Err(JobStoreError::InvalidReceipt);
        }
        snapshot.stream_epoch = self.epoch;
        Ok(snapshot)
    }
    pub(super) fn bump(&self, snapshot: &mut JobSnapshot) -> Result<(), JobStoreError> {
        snapshot.revision = snapshot
            .revision
            .next()
            .map_err(|_| JobStoreError::RevisionOverflow)?;
        snapshot.stream_epoch = self.epoch;
        snapshot.observed_at = Utc::now();
        Ok(())
    }
    pub(super) async fn save(
        &self,
        transaction: &mut Transaction<'_, Sqlite>,
        snapshot: &JobSnapshot,
    ) -> Result<(), JobStoreError> {
        sqlx::query("UPDATE jobs SET phase = ?, snapshot = ? WHERE job_id = ?")
            .bind(snapshot.phase.as_str())
            .bind(serde_json::to_string(snapshot)?)
            .bind(snapshot.job_id.to_string())
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }
}
