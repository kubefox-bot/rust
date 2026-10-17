use super::SubmitError;
use crate::profiles::SelectedProfile;
use hmail_cleaner_protocol::{
    browser::jobs::JobView, JobPhase, JobSnapshot, ObservedMailState, Operation, ServicePolicy,
};
pub(super) fn validate(
    view: &JobView,
    snapshot: &JobSnapshot,
    selected: &SelectedProfile,
) -> Result<(), SubmitError> {
    if snapshot.job_id != view.job_id
        || snapshot.stream_epoch.is_nil()
        || snapshot.revision.get() == 0
    {
        return Err(SubmitError::Protocol);
    }
    match &snapshot.job {
        Some(job)
            if job.agent_id == selected.credentials.agent_id
                && job.profile_id == selected.profile_id
                && job.profile_version == view.intent.profile_version
                && job.operation == view.intent.operation
                && job.scan == view.intent.scan
                && job.resolved_cutoff == view.intent.resolved_cutoff
                && job.limits == view.intent.limits
                && job.service_policy
                    == match view.intent.operation {
                        Operation::Preview => ServicePolicy::RequireStopped,
                        Operation::Clean => view.intent.service_policy,
                    } => {}
        None if snapshot.phase == JobPhase::Cancelled => (),
        _ => return Err(SubmitError::Protocol),
    }
    if let Some(prior) = &view.snapshot {
        if snapshot.stream_epoch == prior.stream_epoch && snapshot.revision < prior.revision {
            return Err(SubmitError::Protocol);
        }
        if !prior.phase.is_active() && snapshot.phase != prior.phase {
            return Err(SubmitError::Protocol);
        }
        if snapshot.stream_epoch == prior.stream_epoch
            && snapshot.revision == prior.revision
            && snapshot != prior
        {
            return Err(SubmitError::Protocol);
        }
        if prior.phase != snapshot.phase && !prior.phase.permits(snapshot.phase) {
            return Err(SubmitError::Protocol);
        }
    }
    Ok(())
}
pub(super) fn safe_to_release(
    view: &JobView,
    snapshot: &JobSnapshot,
    observed: ObservedMailState,
) -> bool {
    if !matches!(
        observed,
        ObservedMailState::Running | ObservedMailState::Stopped
    ) {
        return false;
    }
    if snapshot.job.is_none() {
        return snapshot.phase == JobPhase::Cancelled;
    }
    match (view.intent.operation, view.intent.service_policy) {
        (Operation::Clean, ServicePolicy::Manage) => {
            snapshot.initial_mail_state == Some(observed)
                && (snapshot.mail_state == observed || snapshot.phase == JobPhase::Interrupted)
        }
        (Operation::Preview, _) | (Operation::Clean, ServicePolicy::RequireStopped) => true,
    }
}
