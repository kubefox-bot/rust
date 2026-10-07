use super::descriptor;
use crate::mysql::{ids, transaction, RepositoryError, MAX_BATCH_SIZE};
use hmail_cleaner_core::{
    cleanup::{DeleteResult, DeletedRows, PortResult},
    maintenance::{OrphanRow, OrphanTable},
};
use sqlx::{Connection, MySql, MySqlConnection, QueryBuilder, Row, Transaction};
use std::collections::HashMap;

pub(in crate::mysql) async fn delete(
    connection: &mut MySqlConnection,
    table: OrphanTable,
    rows: &[OrphanRow],
) -> PortResult<DeleteResult> {
    if rows.is_empty() {
        return Ok(DeleteResult::Committed(DeletedRows::default()));
    }
    let expected: HashMap<_, _> = rows.iter().map(|row| (row.id, *row)).collect();
    if expected.len() != rows.len()
        || rows.len() > MAX_BATCH_SIZE
        || rows.iter().any(|row| row.id <= 0)
    {
        return Err(RepositoryError::InvalidBatch("invalid orphan IDs").into());
    }
    let mut tx = connection.begin().await?;
    let affected = match execute(&mut tx, table, &expected).await {
        Ok(affected) => affected,
        Err(error) => return transaction::rollback(tx, error).await,
    };
    let counts = match table {
        OrphanTable::Metadata => DeletedRows {
            metadata: affected,
            ..DeletedRows::default()
        },
        OrphanTable::Recipients => DeletedRows {
            recipients: affected,
            ..DeletedRows::default()
        },
    };
    Ok(transaction::commit(tx, counts).await)
}
async fn execute(
    tx: &mut Transaction<'_, MySql>,
    kind: OrphanTable,
    expected: &HashMap<i64, OrphanRow>,
) -> PortResult<u64> {
    let table = descriptor(kind);
    let mut query = QueryBuilder::<MySql>::new(format!(
        "SELECT c.{id} AS id,c.{parent} AS parent FROM {name} c WHERE c.{id} IN (",
        id = table.id,
        parent = table.parent,
        name = table.name
    ));
    ids::bind(&mut query, expected.keys().copied());
    query.push(format!(
        ") AND NOT EXISTS (SELECT 1 FROM hm_messages m WHERE m.messageid=c.{}) FOR UPDATE",
        table.parent
    ));
    let current = query.build().fetch_all(&mut **tx).await?;
    if current.len() != expected.len() {
        return Err(RepositoryError::StaleBatch.into());
    }
    for row in current {
        let current = OrphanRow {
            id: row.try_get("id")?,
            parent_message_id: row.try_get("parent")?,
        };
        if expected.get(&current.id) != Some(&current) {
            return Err(RepositoryError::StaleBatch.into());
        }
    }
    let mut deletion = QueryBuilder::<MySql>::new(format!(
        "DELETE FROM {} WHERE {} IN (",
        table.name, table.id
    ));
    ids::bind(&mut deletion, expected.keys().copied());
    deletion.push(format!(
        ") AND NOT EXISTS (SELECT 1 FROM hm_messages m WHERE m.messageid={}.{})",
        table.name, table.parent
    ));
    let affected = deletion.build().execute(&mut **tx).await?.rows_affected();
    if affected != expected.len() as u64 {
        return Err(RepositoryError::StaleBatch.into());
    }
    Ok(affected)
}
pub(in crate::mysql) async fn verify(
    connection: &mut MySqlConnection,
    kind: OrphanTable,
    selected: &[i64],
) -> PortResult<()> {
    if selected.is_empty() {
        return Ok(());
    }
    if selected.len() > MAX_BATCH_SIZE {
        return Err(RepositoryError::InvalidBatch("too many orphan IDs").into());
    }
    let table = descriptor(kind);
    let mut query = QueryBuilder::<MySql>::new(format!(
        "SELECT COUNT(*) FROM {} WHERE {} IN (",
        table.name, table.id
    ));
    ids::bind(&mut query, selected.iter().copied());
    query.push(")");
    let count: i64 = query.build_query_scalar().fetch_one(connection).await?;
    if count != 0 {
        return Err(RepositoryError::Verification.into());
    }
    Ok(())
}
