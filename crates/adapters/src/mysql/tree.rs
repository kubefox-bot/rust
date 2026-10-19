use super::*;
use hmail_cleaner_core::{
    cleanup::Cancellation,
    maintenance::{
        recursive::{StoredGroup, TreeRepository},
        StoredMessage,
    },
};
use std::path::{Path, PathBuf};

impl TreeRepository for MySqlRepository {
    async fn prepare_inventory(&mut self, root: &Path, cancel: &Cancellation) -> PortResult<()> {
        // Never expose a partial or stale index after a failed preparation.
        if let Some(index) = self.inventory.take() {
            index.close().await?;
        }
        cancel.check()?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut self.connection)
            .await?;
        let mut transaction = self.connection.begin().await?;
        let upper_id: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(messageid),0) FROM hm_messages")
                .fetch_one(&mut *transaction)
                .await?;
        let registry = registry::load(&mut transaction).await?;
        let index = inventory::load(&mut transaction, &registry, root, upper_id, cancel).await?;
        cancel.check()?;
        transaction.commit().await?;
        self.inventory = Some(index);
        Ok(())
    }
    async fn match_files(&mut self, paths: &[PathBuf]) -> PortResult<Vec<StoredGroup>> {
        self.inventory
            .as_mut()
            .ok_or(RepositoryError::NoSnapshot)?
            .match_files(paths)
            .await
    }
    async fn unmatched_batch(&mut self, cursor: i64, limit: u32) -> PortResult<Vec<StoredGroup>> {
        self.inventory
            .as_mut()
            .ok_or(RepositoryError::NoSnapshot)?
            .unmatched_batch(cursor, limit)
            .await
    }
    async fn delete_stored(&mut self, rows: &[StoredMessage]) -> PortResult<DeleteResult> {
        let candidates = rows.iter().map(|r| r.candidate.clone()).collect::<Vec<_>>();
        let types = rows
            .iter()
            .map(|r| (r.candidate.id, r.message_type))
            .collect();
        delete::batch_with_types(&mut self.connection, &candidates, &types).await
    }
}
