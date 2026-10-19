use hmail_cleaner_core::{
    cleanup::{PortResult, MAX_BATCH_SIZE},
    maintenance::{recursive::StoredGroup, StoredMessage},
    paths::path_key,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteRow, SqliteSynchronous},
    Connection, QueryBuilder, Row, Sqlite, SqliteConnection,
};
use std::path::PathBuf;

#[cfg(test)]
mod tests;

const CACHE_KIB: &str = "-4096";
const STATEMENT_CACHE_CAPACITY: usize = 4;

/// Disposable metadata, isolated from the durable agent ledger. SQLite owns the
/// temporary file and deletes it on close; only a fixed pager cache stays in RAM.
pub(in crate::mysql) struct Index {
    connection: SqliteConnection,
}
impl Index {
    pub(in crate::mysql) async fn create() -> PortResult<Self> {
        let options = SqliteConnectOptions::new()
            .filename("")
            .create_if_missing(true)
            .statement_cache_capacity(STATEMENT_CACHE_CAPACITY)
            .journal_mode(SqliteJournalMode::Off)
            .synchronous(SqliteSynchronous::Off)
            .pragma("temp_store", "FILE")
            .pragma("cache_size", CACHE_KIB)
            .pragma("mmap_size", "0");
        let mut connection = SqliteConnection::connect_with(&options).await?;
        sqlx::query("CREATE TABLE paths (path_key TEXT PRIMARY KEY, message_id INTEGER NOT NULL, record TEXT NOT NULL, refs INTEGER NOT NULL CHECK(refs>0), seen INTEGER NOT NULL DEFAULT 0)")
            .execute(&mut connection).await?;
        sqlx::query("CREATE INDEX unmatched ON paths(seen,message_id)")
            .execute(&mut connection)
            .await?;
        Ok(Self { connection })
    }
    pub(in crate::mysql) async fn insert(&mut self, rows: &[StoredMessage]) -> PortResult<()> {
        if rows.is_empty() {
            return Ok(());
        }
        if rows.len() > MAX_BATCH_SIZE as usize {
            return Err("Слишком большой пакет индекса".into());
        }
        let records = rows
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?;
        let mut query =
            QueryBuilder::<Sqlite>::new("INSERT INTO paths(path_key,message_id,record,refs) ");
        query.push_values(rows.iter().zip(&records), |mut values, (row, record)| {
            values
                .push_bind(path_key(&row.path))
                .push_bind(row.candidate.id)
                .push_bind(record)
                .push("1");
        });
        query.push(" ON CONFLICT(path_key) DO UPDATE SET refs=refs+1");
        query.build().execute(&mut self.connection).await?;
        Ok(())
    }
    pub(in crate::mysql) async fn match_files(
        &mut self,
        paths: &[PathBuf],
    ) -> PortResult<Vec<StoredGroup>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        if paths.len() > MAX_BATCH_SIZE as usize {
            return Err("Слишком большой пакет путей".into());
        }
        let mut query = QueryBuilder::<Sqlite>::new("UPDATE paths SET seen=1 WHERE path_key IN (");
        let mut keys = query.separated(",");
        for path in paths {
            keys.push_bind(path_key(path));
        }
        keys.push_unseparated(") RETURNING record,refs");
        let rows = query.build().fetch_all(&mut self.connection).await?;
        rows.into_iter().map(decode).collect()
    }
    pub(in crate::mysql) async fn unmatched_batch(
        &mut self,
        cursor: i64,
        limit: u32,
    ) -> PortResult<Vec<StoredGroup>> {
        if cursor < 0 || limit == 0 || limit > MAX_BATCH_SIZE {
            return Err("Некорректный пакет отсутствующих файлов".into());
        }
        let rows = sqlx::query("SELECT record,refs FROM paths WHERE seen=0 AND message_id>? ORDER BY message_id LIMIT ?")
            .bind(cursor).bind(limit).fetch_all(&mut self.connection).await?;
        rows.into_iter().map(decode).collect()
    }
    pub(in crate::mysql) async fn close(self) -> PortResult<()> {
        self.connection.close().await?;
        Ok(())
    }
}
fn decode(row: SqliteRow) -> PortResult<StoredGroup> {
    Ok(StoredGroup {
        row: serde_json::from_str(row.try_get("record")?)?,
        references: u64::try_from(row.try_get::<i64, _>("refs")?)?,
    })
}
