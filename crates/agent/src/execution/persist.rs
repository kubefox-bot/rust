use super::{mapping, observer::ProgressValue, AgentError, Executor, Worker};
use crate::jobs::JobUpdate;
use hmail_cleaner_core::maintenance::mail_service::MailState;
use hmail_cleaner_protocol::{ErrorCode, JobId, JobPhase, ObservedMailState, PublicError};
use hmail_cleaner_runner::{MailObservation, Report, Status};

impl<E: Executor> Worker<E> {
    pub(super) async fn observe(
        &self,
        id: JobId,
        progress: Option<ProgressValue>,
        service: Option<MailObservation>,
    ) -> Result<(), AgentError> {
        let snapshot = self
            .0
            .repository
            .observe(
                id,
                progress.map(|value| (value.stage, value.summary)),
                service.map(|value| match value {
                    MailObservation::Restoring => ObservedMailState::Restoring,
                    MailObservation::State(MailState::Running) => ObservedMailState::Running,
                    MailObservation::State(MailState::Stopped) => ObservedMailState::Stopped,
                    MailObservation::State(MailState::Transitioning | MailState::Unsupported) => {
                        ObservedMailState::Unknown
                    }
                }),
            )
            .await?;
        let _ = self.0.events.send(snapshot);
        Ok(())
    }
    pub(super) async fn finish(
        &self,
        id: JobId,
        report: Report,
        fatal: bool,
    ) -> Result<(), AgentError> {
        let prior = self.lookup(id).await?.ok_or(AgentError::Unavailable)?;
        let restoration_unconfirmed = prior.mail_state == ObservedMailState::Restoring;
        let phase = if fatal || restoration_unconfirmed {
            JobPhase::Failed
        } else {
            match report.status {
                Status::Complete => JobPhase::Completed,
                Status::Partial => JobPhase::Partial,
                Status::Cancelled => JobPhase::Cancelled,
                Status::Failed | Status::Invalid => JobPhase::Failed,
            }
        };
        let error = if restoration_unconfirmed {
            Some(PublicError::new(ErrorCode::RestoreFailed))
        } else {
            match phase {
                JobPhase::Failed | JobPhase::Partial => {
                    Some(PublicError::new(ErrorCode::CleanupFailed))
                }
                JobPhase::Cancelled => Some(PublicError::new(ErrorCode::Cancelled)),
                _ => None,
            }
        };
        let snapshot = self
            .0
            .repository
            .transition(
                id,
                JobUpdate {
                    phase,
                    stage: report.stage.map(mapping::stage),
                    summary: mapping::summary(&report.summary),
                    mail_state: if restoration_unconfirmed {
                        ObservedMailState::Unknown
                    } else {
                        prior.mail_state
                    },
                    error,
                },
            )
            .await?;
        let _ = self.0.events.send(snapshot);
        Ok(())
    }
}
