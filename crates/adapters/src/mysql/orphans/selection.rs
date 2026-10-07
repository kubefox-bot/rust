use super::descriptor;
use crate::mysql::{RepositoryError, MAX_BATCH_SIZE};
use hmail_cleaner_core::{
    cleanup::PortResult,
    maintenance::{OrphanBatch, OrphanRow, OrphanSelection, OrphanTable},
};
use sqlx::{MySqlConnection, Row};

pub(in crate::mysql) async fn upper(
    connection: &mut MySqlConnection,
    table: OrphanTable,
) -> PortResult<i64> {
    let table = descriptor(table);
    let value: Option<i64> =
        sqlx::query_scalar(&format!("SELECT MAX({}) FROM {}", table.id, table.name))
            .fetch_one(connection)
            .await?;
    Ok(value.unwrap_or(0))
}
pub(in crate::mysql) async fn next(
    connection: &mut MySqlConnection,
    selection: OrphanSelection,
    cursor: i64,
    limit: u32,
) -> PortResult<OrphanBatch> {
    if limit == 0 || limit as usize > MAX_BATCH_SIZE || cursor < 0 || selection.upper_id < cursor {
        return Err(RepositoryError::InvalidBatch("invalid orphan selection bounds").into());
    }
    let query = statement(selection.table);
    let rows = sqlx::query(&query)
        .bind(cursor)
        .bind(selection.upper_id)
        .bind(limit)
        .fetch_all(connection)
        .await?;
    let rows = rows
        .into_iter()
        .map(|row| {
            Ok(OrphanRow {
                id: row.try_get("id")?,
                parent_message_id: row.try_get("parent")?,
            })
        })
        .collect::<PortResult<Vec<_>>>()?;
    Ok(OrphanBatch {
        last_id: rows.last().map_or(cursor, |row| row.id),
        rows,
    })
}

fn statement(kind: OrphanTable) -> String {
    let table = descriptor(kind);
    format!("SELECT c.{id} AS id,c.{parent} AS parent FROM {name} c FORCE INDEX(PRIMARY)
        WHERE c.{id}>? AND c.{id}<=? AND NOT EXISTS (SELECT 1 FROM hm_messages m WHERE m.messageid=c.{parent})
        ORDER BY c.{id} LIMIT ?", id=table.id, parent=table.parent, name=table.name)
}

#[cfg(test)]
#[path = "../../../tests/fixtures/mod.rs"]
mod fixtures;

#[cfg(test)]
mod tests {
    use super::*;
    use hmail_cleaner_core::maintenance::FullRepository;
    use sqlx::Column;

    #[tokio::test]
    #[ignore = "requires disposable MySQL"]
    async fn orphan_selection_uses_primary_range_and_parent_lookup() {
        let mut fixture = fixtures::Fixture::create().await;
        fixture
            .add(1, 1, 10, 2, "2026-01-01 10:00:00", &fixtures::name(1))
            .await;
        fixture.add_dependents(1).await;
        fixture.add_dependents(9).await;
        let mut repository = fixture.repository().await;
        let upper_id = repository
            .orphan_upper_id(OrphanTable::Metadata)
            .await
            .unwrap();
        let selected = repository
            .orphan_batch(
                OrphanSelection {
                    table: OrphanTable::Metadata,
                    upper_id,
                },
                0,
                1,
            )
            .await
            .unwrap();
        assert_eq!(selected.rows[0].parent_message_id, 9);
        repository.close().await.unwrap();
        let (reader_user, reader) = fixture.read_only_config().await;
        let mut readonly = crate::mysql::MySqlRepository::connect(&reader)
            .await
            .unwrap();
        let read_batch = readonly
            .orphan_batch(
                OrphanSelection {
                    table: OrphanTable::Metadata,
                    upper_id,
                },
                0,
                1,
            )
            .await
            .unwrap();
        assert_eq!(read_batch.rows[0].parent_message_id, 9);
        readonly.close().await.unwrap();
        fixture.drop_reader(&reader_user).await;
        let explain = sqlx::query(&format!("EXPLAIN {}", statement(OrphanTable::Metadata)))
            .bind(0_i64)
            .bind(upper_id)
            .bind(1_u32)
            .fetch_all(&mut fixture.admin)
            .await
            .unwrap();
        let value = |row: &sqlx::mysql::MySqlRow, name: &str| {
            let index = row
                .columns()
                .iter()
                .position(|col| col.name() == name)
                .unwrap();
            row.get::<Option<String>, _>(index).unwrap_or_default()
        };
        let outer = explain
            .iter()
            .find(|row| value(row, "table") == "c")
            .unwrap();
        assert_eq!(value(outer, "key"), "PRIMARY");
        assert_eq!(value(outer, "type"), "range");
        assert!(!value(outer, "Extra").contains("filesort"));
        let parent = explain
            .iter()
            .find(|row| value(row, "table") == "m")
            .unwrap();
        assert_eq!(value(parent, "key"), "PRIMARY");
        assert_eq!(
            fixture
                .ids("hm_message_metadata", "metadata_messageid")
                .await,
            vec![1, 9]
        );
        fixture.finish().await;
    }
}
