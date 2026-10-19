use super::types::{FileReferences, OrphanBatch, OrphanRow, OrphanSelection, OrphanTable};
use crate::cleanup::{Cancellation, DeleteResult, PortResult, Repository};
use std::future::Future;

pub trait FullRepository: Repository {
    fn orphan_upper_id(
        &mut self,
        table: OrphanTable,
    ) -> impl Future<Output = PortResult<i64>> + Send;
    fn orphan_batch(
        &mut self,
        selection: OrphanSelection,
        cursor: i64,
        limit: u32,
    ) -> impl Future<Output = PortResult<OrphanBatch>> + Send;
    fn delete_orphans(
        &mut self,
        table: OrphanTable,
        rows: &[OrphanRow],
    ) -> impl Future<Output = PortResult<DeleteResult>> + Send;
    fn verify_orphans_deleted(
        &mut self,
        table: OrphanTable,
        ids: &[i64],
    ) -> impl Future<Output = PortResult<()>> + Send;
    fn file_references(
        &mut self,
        limit: usize,
    ) -> impl Future<Output = PortResult<FileReferences>> + Send;
}

pub trait FileScan: Send {
    fn next_batch(
        &mut self,
        limit: u32,
    ) -> impl Future<Output = PortResult<super::types::ScanBatch>> + Send;
    fn close(self) -> impl Future<Output = PortResult<()>> + Send;
}
pub trait FullFileStore: crate::cleanup::FileStore {
    type Scan: FileScan;
    fn prune_completed(
        &self,
        paths: Vec<std::path::PathBuf>,
    ) -> impl Future<Output = PortResult<crate::cleanup::PruneOutcome>> + Send {
        self.prune_empty(paths)
    }
    fn validate_tree(
        &self,
        cancel: &Cancellation,
    ) -> impl Future<Output = PortResult<super::types::ScanStats>> + Send;
    fn open_scan(&self) -> impl Future<Output = PortResult<Self::Scan>> + Send;
    fn delete_orphans(
        &self,
        files: Vec<super::types::FileObservation>,
        workers: u8,
    ) -> impl Future<Output = PortResult<Vec<super::types::OrphanFileOutcome>>> + Send;
}
