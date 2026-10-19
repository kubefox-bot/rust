use crate::{
    limits::{MAX_UTC_OFFSET_SECONDS, PROTOCOL_VERSION},
    AgentId, Counter, JobId, MailProfile, ProfileId, ProtocolError, PublicError, RunLimits,
    SummaryDto,
};
use chrono::{DateTime, Datelike, NaiveDateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Preview,
    Clean,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scan {
    Fast,
    Full,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServicePolicy {
    RequireStopped,
    Manage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedCutoff {
    pub local: NaiveDateTime,
    pub file_cutoff_utc: DateTime<Utc>,
    pub utc_offset_seconds: i32,
    pub resolved_at_utc: DateTime<Utc>,
}
impl ResolvedCutoff {
    pub fn validate(self) -> Result<Self, ProtocolError> {
        if self.is_valid() {
            Ok(self)
        } else {
            Err(ProtocolError::InvalidJob)
        }
    }
    fn is_valid(self) -> bool {
        (1000..=9999).contains(&self.local.year())
            && self.utc_offset_seconds.unsigned_abs() < MAX_UTC_OFFSET_SECONDS as u32
            && self
                .file_cutoff_utc
                .naive_utc()
                .checked_add_signed(chrono::Duration::seconds(i64::from(
                    self.utc_offset_seconds,
                )))
                == Some(self.local)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobCommand {
    pub protocol_version: u16,
    pub job_id: JobId,
    pub agent_id: AgentId,
    pub profile_id: ProfileId,
    pub profile_version: Counter,
    pub operation: Operation,
    pub scan: Scan,
    pub resolved_cutoff: ResolvedCutoff,
    pub service_policy: ServicePolicy,
    pub limits: RunLimits,
    pub mail_profile: MailProfile,
}
pub struct ValidatedJob(JobCommand);
impl ValidatedJob {
    pub fn command(&self) -> &JobCommand {
        &self.0
    }
    pub fn into_command(self) -> JobCommand {
        self.0
    }
    pub fn public(&self) -> PublicJob {
        let command = &self.0;
        PublicJob {
            agent_id: command.agent_id,
            profile_id: command.profile_id,
            profile_version: command.profile_version,
            operation: command.operation,
            scan: command.scan,
            resolved_cutoff: command.resolved_cutoff,
            service_policy: command.service_policy,
            limits: command.limits,
        }
    }
}
pub fn validate_job(command: JobCommand) -> Result<ValidatedJob, ProtocolError> {
    if command.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion);
    }
    if command.profile_version.get() == 0
        || !command.limits.is_valid()
        || !command.resolved_cutoff.is_valid()
        || (command.operation == Operation::Preview
            && command.service_policy == ServicePolicy::Manage)
    {
        return Err(ProtocolError::InvalidJob);
    }
    if !command.mail_profile.is_valid() {
        return Err(ProtocolError::InvalidProfile);
    }
    Ok(ValidatedJob(command))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicJob {
    pub agent_id: AgentId,
    pub profile_id: ProfileId,
    pub profile_version: Counter,
    pub operation: Operation,
    pub scan: Scan,
    pub resolved_cutoff: ResolvedCutoff,
    pub service_policy: ServicePolicy,
    pub limits: RunLimits,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobPhase {
    Accepted,
    Running,
    Cancelling,
    Restoring,
    Completed,
    Partial,
    Failed,
    Cancelled,
    Interrupted,
}
impl JobPhase {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::Running | Self::Cancelling | Self::Restoring
        )
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Running => "running",
            Self::Cancelling => "cancelling",
            Self::Restoring => "restoring",
            Self::Completed => "completed",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
    pub fn permits(self, next: Self) -> bool {
        match self {
            Self::Accepted => matches!(
                next,
                Self::Running
                    | Self::Cancelling
                    | Self::Cancelled
                    | Self::Interrupted
                    | Self::Failed
            ),
            Self::Running => matches!(
                next,
                Self::Running
                    | Self::Cancelling
                    | Self::Restoring
                    | Self::Completed
                    | Self::Partial
                    | Self::Failed
                    | Self::Cancelled
                    | Self::Interrupted
            ),
            Self::Cancelling => matches!(
                next,
                Self::Cancelling
                    | Self::Restoring
                    | Self::Completed
                    | Self::Partial
                    | Self::Failed
                    | Self::Cancelled
                    | Self::Interrupted
            ),
            Self::Restoring => matches!(
                next,
                Self::Restoring
                    | Self::Completed
                    | Self::Partial
                    | Self::Failed
                    | Self::Cancelled
                    | Self::Interrupted
            ),
            Self::Completed
            | Self::Partial
            | Self::Failed
            | Self::Cancelled
            | Self::Interrupted => false,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobStage {
    Preflight,
    Snapshot,
    Selection,
    Files,
    Database,
    Verification,
    Directories,
    OrphanDatabase,
    References,
    Scan,
    Complete,
    Cancelled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObservedMailState {
    Unknown,
    Running,
    Stopped,
    Restoring,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JobSnapshot {
    pub job_id: JobId,
    pub job: Option<PublicJob>,
    pub stream_epoch: Uuid,
    pub revision: Counter,
    pub phase: JobPhase,
    pub stage: Option<JobStage>,
    pub summary: SummaryDto,
    pub mail_state: ObservedMailState,
    #[serde(default)]
    pub initial_mail_state: Option<ObservedMailState>,
    pub error: Option<PublicError>,
    pub observed_at: DateTime<Utc>,
}
impl From<hmail_cleaner_core::maintenance::mail_service::MailState> for ObservedMailState {
    fn from(state: hmail_cleaner_core::maintenance::mail_service::MailState) -> Self {
        use hmail_cleaner_core::maintenance::mail_service::MailState;
        match state {
            MailState::Running => Self::Running,
            MailState::Stopped => Self::Stopped,
            MailState::Transitioning | MailState::Unsupported => Self::Unknown,
        }
    }
}
