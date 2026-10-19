use hmail_cleaner_protocol::{JobPhase, JobStage, ObservedMailState, PublicError, SummaryDto};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptOutcome {
    New,
    Existing,
    Conflict,
    Busy,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestFingerprint(pub(super) [u8; 32]);

#[derive(Debug, thiserror::Error)]
pub enum JobStoreError {
    #[error("job ledger storage failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("job ledger migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("job ledger file operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid job receipt: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid job receipt")]
    InvalidReceipt,
    #[error("invalid phase transition")]
    InvalidTransition,
    #[error("job does not exist")]
    NotFound,
    #[error("preview file metadata was not captured for this job")]
    PreviewUnavailable,
    #[error("invalid or insufficiently protected receipt key")]
    InvalidKey,
    #[error("random source unavailable")]
    Random,
    #[error("revision exhausted")]
    RevisionOverflow,
}

pub struct JobUpdate {
    pub phase: JobPhase,
    pub stage: Option<JobStage>,
    pub summary: SummaryDto,
    pub mail_state: ObservedMailState,
    pub error: Option<PublicError>,
}
