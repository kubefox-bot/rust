use super::{JobService, SubmitError};
use crate::auth::SessionLease;
use hmail_cleaner_protocol::{browser::jobs::SubmitRequest, JobPhase, Operation};
impl JobService {
    pub(super) async fn validate_preview(
        &self,
        input: &SubmitRequest,
        lease: &SessionLease,
    ) -> Result<(), SubmitError> {
        match (input.operation, input.preview_job_id, input.confirmed) {
            (Operation::Preview, None, false) => Ok(()),
            (Operation::Clean, Some(id), true) => {
                let previous = self.get(id, lease).await?;
                let snapshot = previous.snapshot.ok_or(SubmitError::Invalid)?;
                let prior = previous.intent;
                if previous.slot_held
                    || snapshot.phase != JobPhase::Completed
                    || snapshot.summary.failed.get() != 0
                    || prior.operation != Operation::Preview
                    || prior.server_id != input.server_id
                    || prior.profile_version != input.profile_version
                    || prior.scan != input.scan
                    || prior.resolved_cutoff != input.resolved_cutoff
                    || prior.service_policy != input.service_policy
                    || prior.limits != input.limits
                {
                    return Err(SubmitError::Invalid);
                }
                Ok(())
            }
            _ => Err(SubmitError::Invalid),
        }
    }
}
