//! Versioned, bounded wire contracts. Mail credentials have no Debug implementation.
pub mod browser;
mod canonical;
mod errors;
mod events;
mod ids;
mod jobs;
mod limits;
mod profiles;

pub use canonical::canonical_job;
pub use errors::{ErrorCode, ProtocolError, PublicError};
pub use events::{Counter, EventEnvelope, SummaryDto};
pub use ids::{AgentId, JobId, ProfileId, ServerId};
pub use jobs::{
    validate_job, JobCommand, JobPhase, JobSnapshot, JobStage, ObservedMailState, Operation,
    PublicJob, ResolvedCutoff, Scan, ServicePolicy, ValidatedJob,
};
pub use limits::{RunLimits, PROTOCOL_VERSION};
pub use profiles::{DatabaseParts, MailProfile};
