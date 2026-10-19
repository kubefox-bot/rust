use super::*;
use std::path::PathBuf;

/// One representative row per physical path. Ambiguous groups must never be deleted.
#[derive(Debug, Clone)]
pub struct StoredGroup {
    pub row: StoredMessage,
    pub references: u64,
}

pub trait TreeRepository: FullRepository {
    /// Validate every row and prepare a bounded-memory path index before any deletion.
    /// A new preparation replaces the previous index, including its seen markers.
    fn prepare_inventory(
        &mut self,
        root: &Path,
        cancel: &Cancellation,
    ) -> impl Future<Output = PortResult<()>> + Send;
    /// Return at most one group per requested path and mark all matches as seen.
    /// Seen groups must not appear in unmatched_batch, even if file deletion fails.
    fn match_files(
        &mut self,
        paths: &[PathBuf],
    ) -> impl Future<Output = PortResult<Vec<StoredGroup>>> + Send;
    /// Unseen groups in increasing representative message-ID order, bounded by limit.
    fn unmatched_batch(
        &mut self,
        cursor: i64,
        limit: u32,
    ) -> impl Future<Output = PortResult<Vec<StoredGroup>>> + Send;
    fn delete_stored(
        &mut self,
        rows: &[StoredMessage],
    ) -> impl Future<Output = PortResult<DeleteResult>> + Send;
}
