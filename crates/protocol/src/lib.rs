//! Versioned, bounded wire contracts. Mail credentials have no Debug implementation.
mod agent;
pub mod browser;
mod canonical;
mod errors;
mod events;
mod ids;
mod jobs;
mod limits;
mod preview_files;
mod profiles;

pub use agent::{
    AgentCsr, AgentIdentityInstall, AgentIdentityRequest, IdentityReceipt, InstallationStatus,
};
pub use agent::{
    AgentHealth, ProfileValidationRequest, ProfileValidationResponse, ResolveRequest,
    RetentionRequest, ServiceStateRequest, ServiceStateResponse, ValidatedProfile,
};
pub use canonical::canonical_job;
pub use errors::{ErrorCode, ProtocolError, PublicError};
pub use events::{Counter, EventEnvelope, SummaryDto};
pub use ids::{AgentId, JobId, ProfileId, ServerId};
pub use jobs::{
    validate_job, JobCommand, JobPhase, JobSnapshot, JobStage, ObservedMailState, Operation,
    PublicJob, ResolvedCutoff, Scan, ServicePolicy, ValidatedJob,
};
pub use limits::{RunLimits, PROTOCOL_VERSION};
pub use preview_files::{
    PreviewFile, PreviewFileSort, PreviewFilesPage, PreviewFilesQuery, SortDirection,
    PREVIEW_FILES_DEFAULT_LIMIT, PREVIEW_FILES_MAX_LIMIT, PREVIEW_FILES_MAX_SEARCH_BYTES,
    PREVIEW_FILE_MAX_PATH_BYTES,
};
pub use profiles::{DatabaseParts, DatabaseTls, MailProfile};
