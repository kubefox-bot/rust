use std::{error::Error, future::Future, path::PathBuf};

use super::types::{
    Batch, Candidate, DeleteResult, FileOutcome, FileTarget, PruneOutcome, Selection, Snapshot,
};

pub type PortResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub trait Repository {
    fn snapshot(&mut self) -> impl Future<Output = PortResult<Snapshot>> + Send;
    fn next_batch(
        &mut self,
        selection: Selection,
        cursor: i64,
        limit: u32,
    ) -> impl Future<Output = PortResult<Batch>> + Send;
    fn delete_batch(
        &mut self,
        candidates: &[Candidate],
    ) -> impl Future<Output = PortResult<DeleteResult>> + Send;
    fn verify_deleted(&mut self, ids: &[i64]) -> impl Future<Output = PortResult<()>> + Send;
}

pub trait FileStore {
    fn delete(
        &self,
        files: Vec<FileTarget>,
        workers: u8,
    ) -> impl Future<Output = PortResult<Vec<FileOutcome>>> + Send;
    fn prune_empty(
        &self,
        parents: Vec<PathBuf>,
    ) -> impl Future<Output = PortResult<PruneOutcome>> + Send;
}

pub trait OfflineGuard {
    fn ensure_stopped(&self) -> impl Future<Output = PortResult<()>> + Send;
}
