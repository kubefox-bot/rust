use super::{keys, JobStoreError};
use hmail_cleaner_protocol::{JobId, JobSnapshot};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    Sqlite, SqlitePool, Transaction,
};
use std::{path::Path, time::Duration};
use uuid::Uuid;
use zeroize::Zeroizing;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
pub struct JobRepository {
    pub(super) pool: SqlitePool,
    pub(super) epoch: Uuid,
    pub(super) key: Zeroizing<[u8; 32]>,
}
impl JobRepository {
    pub fn epoch(&self) -> Uuid {
        self.epoch
    }
    pub async fn open(path: &Path) -> Result<Self, JobStoreError> {
        let key = keys::load_or_create(path)?;
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(BUSY_TIMEOUT);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self {
            pool,
            epoch: Uuid::new_v4(),
            key,
        })
    }

    pub async fn lookup(&self, id: JobId) -> Result<Option<JobSnapshot>, JobStoreError> {
        sqlx::query("SELECT job_id, phase, snapshot FROM jobs WHERE job_id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?
            .map(|row| self.decode(&row, id))
            .transpose()
    }

    pub async fn close(self) {
        self.pool.close().await;
    }

    pub(super) async fn writer(&self) -> Result<Transaction<'_, Sqlite>, JobStoreError> {
        let mut transaction = self.pool.begin().await?;
        // Acquire SQLite's writer reservation before inspecting admission state.
        sqlx::query("UPDATE coordination SET revision = revision WHERE id = 1")
            .execute(&mut *transaction)
            .await?;
        Ok(transaction)
    }
}
