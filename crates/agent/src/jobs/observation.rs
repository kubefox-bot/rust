use super::{JobRepository, JobStoreError};
use hmail_cleaner_protocol::{
    JobId, JobPhase, JobSnapshot, JobStage, ObservedMailState, SummaryDto,
};
impl JobRepository {
    /// Serializes progress with cancellation so observations cannot undo Cancelling.
    pub async fn observe(
        &self,
        id: JobId,
        progress: Option<(JobStage, SummaryDto)>,
        mail: Option<ObservedMailState>,
    ) -> Result<JobSnapshot, JobStoreError> {
        let mut transaction = self.writer().await?;
        let row = sqlx::query("SELECT job_id, phase, snapshot FROM jobs WHERE job_id = ?")
            .bind(id.to_string())
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(JobStoreError::NotFound)?;
        let mut snapshot = self.decode(&row, id)?;
        if !snapshot.phase.is_active() {
            return Err(JobStoreError::InvalidTransition);
        }
        if let Some((stage, summary)) = progress {
            snapshot.stage = Some(stage);
            snapshot.summary = summary;
        }
        if let Some(state) = mail {
            if snapshot.initial_mail_state.is_none()
                && matches!(
                    state,
                    ObservedMailState::Running | ObservedMailState::Stopped
                )
            {
                snapshot.initial_mail_state = Some(state);
            }
            snapshot.mail_state = state;
            if state == ObservedMailState::Restoring {
                snapshot.phase = JobPhase::Restoring;
            }
        }
        self.bump(&mut snapshot)?;
        self.save(&mut transaction, &snapshot).await?;
        transaction.commit().await?;
        Ok(snapshot)
    }
}
