mod batch;
mod file_results;
mod limits;
mod ports;
mod progress;
mod types;
mod workflow;
pub use limits::{DEFAULT_BATCH_SIZE, DEFAULT_WORKERS, MAX_BATCH_SIZE, MAX_WORKERS};
pub use progress::{Cancellation, CleanupError, Failure, Progress, Stage};
pub use workflow::{clean, preview, Dependencies};

pub use ports::{FileStore, OfflineGuard, PortResult, Repository};
pub use types::{
    Batch, Candidate, DeleteResult, DeletionResult, DirectoryFailure, FileFailure, FileFailureKind,
    FileOutcome, FileTarget, PruneOutcome, RowFailure, RunOptions, Selection, Snapshot, Summary,
};
