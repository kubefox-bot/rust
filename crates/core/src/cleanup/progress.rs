use super::{PortResult, Summary};
use serde::Serialize;
use std::{
    error::Error,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Default)]
pub struct Cancellation(AtomicBool);
impl Cancellation {
    pub fn request(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_requested(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Preflight,
    Snapshot,
    Selection,
    Files,
    Database,
    Verification,
    Directories,
    Complete,
    Cancelled,
    OrphanDatabase,
    References,
    Scan,
}
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Failure {
    Message { id: i64, reason: String },
    Directory { path: PathBuf, reason: String },
    OrphanFile { path: PathBuf, reason: String },
}
#[derive(Debug, Serialize)]
pub struct Progress {
    pub stage: Stage,
    pub summary: Summary,
    pub failure: Option<Failure>,
}
#[derive(Debug, thiserror::Error)]
#[error("{stage:?}: {source}")]
pub struct CleanupError {
    pub stage: Stage,
    pub summary: Box<Summary>,
    #[source]
    pub source: Box<dyn Error + Send + Sync>,
}
impl CleanupError {
    pub fn is_cancelled(&self) -> bool {
        self.stage == Stage::Cancelled
    }
}
pub(super) fn at<T>(
    result: PortResult<T>,
    stage: Stage,
    summary: Summary,
) -> Result<T, CleanupError> {
    result.map_err(|source| CleanupError {
        stage,
        summary: Box::new(summary),
        source,
    })
}
