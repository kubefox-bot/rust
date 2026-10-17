use super::{JobRepository, JobStoreError};
use chrono::Utc;
use hmail_cleaner_protocol::{
    Counter, JobId, JobPhase, JobSnapshot, ObservedMailState, SummaryDto,
};
impl JobRepository {
    pub async fn cancel(&self, id: JobId) -> Result<JobSnapshot, JobStoreError> {
        let mut transaction = self.writer().await?;
        let row = sqlx::query("SELECT job_id, phase, snapshot FROM jobs WHERE job_id = ?")
            .bind(id.to_string())
            .fetch_optional(&mut *transaction)
            .await?;
        let snapshot = match row {
            Some(row) => {
                let mut snapshot = self.decode(&row, id)?;
                if !snapshot.phase.is_active() {
                    transaction.commit().await?;
                    return Ok(snapshot);
                }
                snapshot.phase = match snapshot.phase {
                    JobPhase::Accepted => JobPhase::Cancelled,
                    JobPhase::Running | JobPhase::Cancelling => JobPhase::Cancelling,
                    JobPhase::Restoring => JobPhase::Restoring,
                    JobPhase::Completed
                    | JobPhase::Partial
                    | JobPhase::Failed
                    | JobPhase::Cancelled
                    | JobPhase::Interrupted => unreachable!("guarded terminal state"),
                };
                self.bump(&mut snapshot)?;
                self.save(&mut transaction, &snapshot).await?;
                snapshot
            }
            None => {
                let snapshot = JobSnapshot {
                    job_id: id,
                    job: None,
                    stream_epoch: self.epoch,
                    revision: Counter::from(1),
                    phase: JobPhase::Cancelled,
                    stage: None,
                    summary: SummaryDto::default(),
                    mail_state: ObservedMailState::Unknown,
                    initial_mail_state: None,
                    error: None,
                    observed_at: Utc::now(),
                };
                sqlx::query("INSERT INTO jobs (job_id, phase, snapshot) VALUES (?, ?, ?)")
                    .bind(id.to_string())
                    .bind(snapshot.phase.as_str())
                    .bind(serde_json::to_string(&snapshot)?)
                    .execute(&mut *transaction)
                    .await?;
                snapshot
            }
        };
        transaction.commit().await?;
        Ok(snapshot)
    }
}
