use super::JobStoreError;
use sqlx::{Sqlite, Transaction};

const RETAINED_PREVIEW_CATALOGS: i64 = 3;
const EXPIRED_CATALOGS: &str =
    "SELECT c.job_id FROM preview_catalogs c JOIN jobs j ON j.job_id = c.job_id
     WHERE j.phase IN ('completed','partial','failed','cancelled','interrupted')
     ORDER BY c.rowid DESC LIMIT -1 OFFSET ?";

/// Catalog admission order is stable across restarts; active catalogs and receipts stay intact.
pub(super) async fn prune(transaction: &mut Transaction<'_, Sqlite>) -> Result<(), JobStoreError> {
    let expired: Vec<String> = sqlx::query_scalar(EXPIRED_CATALOGS)
        .bind(RETAINED_PREVIEW_CATALOGS)
        .fetch_all(&mut **transaction)
        .await?;
    for id in expired {
        sqlx::query("DELETE FROM preview_files WHERE job_id = ?")
            .bind(&id)
            .execute(&mut **transaction)
            .await?;
        sqlx::query("DELETE FROM preview_catalogs WHERE job_id = ?")
            .bind(id)
            .execute(&mut **transaction)
            .await?;
    }
    Ok(())
}
