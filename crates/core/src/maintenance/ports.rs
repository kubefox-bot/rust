use super::types::{FileReferences, OrphanBatch, OrphanRow, OrphanSelection, OrphanTable};
use crate::cleanup::{DeleteResult, PortResult, Repository};
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
