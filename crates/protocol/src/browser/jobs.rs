use crate::{
    Counter, JobId, JobSnapshot, Operation, ResolvedCutoff, RunLimits, Scan, ServerId,
    ServicePolicy,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmitRequest {
    pub job_id: JobId,
    pub server_id: ServerId,
    pub profile_version: Counter,
    pub operation: Operation,
    pub scan: Scan,
    pub resolved_cutoff: ResolvedCutoff,
    /// Intended cleanup policy; preview itself never stops the service.
    pub service_policy: ServicePolicy,
    pub limits: RunLimits,
    pub preview_job_id: Option<JobId>,
    pub confirmed: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DispatchState {
    Unknown,
    Acknowledged,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JobView {
    pub job_id: JobId,
    pub intent: SubmitRequest,
    pub dispatch: DispatchState,
    pub snapshot: Option<JobSnapshot>,
    pub slot_held: bool,
    pub stream_epoch: Uuid,
    pub revision: Counter,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryPage {
    pub jobs: Vec<JobView>,
    pub next_cursor: Option<Counter>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JobEvent {
    pub stream_epoch: Uuid,
    pub revision: Counter,
    pub job: JobView,
}
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CleanupContext {
    pub server: super::profiles::ServerView,
    pub profile_version: Counter,
    pub configured: bool,
    pub latest: Option<JobView>,
    pub busy: bool,
}
