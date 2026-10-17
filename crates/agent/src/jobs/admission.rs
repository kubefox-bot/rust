use super::{AcceptOutcome, JobRepository, JobStoreError, RequestFingerprint};
use chrono::Utc;
use hmail_cleaner_protocol::{
    Counter, JobPhase, JobSnapshot, ObservedMailState, SummaryDto, ValidatedJob,
};
use sqlx::Row;
use subtle::ConstantTimeEq;
const ACTIVE_PHASES: &str = "SELECT job_id FROM jobs WHERE phase IN ('accepted','running','cancelling','restoring') LIMIT 1";
impl JobRepository {
    pub async fn accept(
        &self,
        job: &ValidatedJob,
        fingerprint: RequestFingerprint,
    ) -> Result<AcceptOutcome, JobStoreError> {
        let mut transaction = self.writer().await?;
        let id = job.command().job_id;
        let existing =
            sqlx::query("SELECT job_id, fingerprint, phase, snapshot FROM jobs WHERE job_id = ?")
                .bind(id.to_string())
                .fetch_optional(&mut *transaction)
                .await?;
        if let Some(row) = existing {
            let snapshot = self.decode(&row, id)?;
            let stored: Option<Vec<u8>> = row.try_get("fingerprint")?;
            transaction.commit().await?;
            if snapshot.phase == JobPhase::Cancelled && stored.is_none() {
                return Ok(AcceptOutcome::Cancelled);
            }
            return Ok(match stored {
                Some(value) if value.as_slice().ct_eq(&fingerprint.0).into() => {
                    AcceptOutcome::Existing
                }
                _ => AcceptOutcome::Conflict,
            });
        }
        if sqlx::query(ACTIVE_PHASES)
            .fetch_optional(&mut *transaction)
            .await?
            .is_some()
        {
            transaction.commit().await?;
            return Ok(AcceptOutcome::Busy);
        }
        let snapshot = JobSnapshot {
            job_id: id,
            job: Some(job.public()),
            stream_epoch: self.epoch,
            revision: Counter::from(1),
            phase: JobPhase::Accepted,
            stage: None,
            summary: SummaryDto::default(),
            mail_state: ObservedMailState::Unknown,
            error: None,
            observed_at: Utc::now(),
        };
        sqlx::query("INSERT INTO jobs (job_id, fingerprint, phase, snapshot) VALUES (?, ?, ?, ?)")
            .bind(id.to_string())
            .bind(fingerprint.0.to_vec())
            .bind(snapshot.phase.as_str())
            .bind(serde_json::to_string(&snapshot)?)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(AcceptOutcome::New)
    }
}
