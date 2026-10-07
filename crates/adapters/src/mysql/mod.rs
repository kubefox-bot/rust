mod config;
mod delete;
mod duplicates;
mod registry;
mod schema;
mod select;

pub use config::{DatabaseConfig, DatabaseConfigError};

use hmail_cleaner_core::cleanup::{
    Batch, Candidate, DeleteResult, PortResult, Repository, Selection, Snapshot,
};
use registry::Registry;
use sqlx::{Connection, MySqlConnection};
use std::collections::HashSet;
use thiserror::Error;

const DELIVERED_STATE: i8 = 2;
pub(super) const MAX_BATCH_SIZE: usize = hmail_cleaner_core::cleanup::MAX_BATCH_SIZE as usize;
const LOCK_QUERY: &str = "SELECT GET_LOCK(CONCAT('hmail-clean:', LEFT(SHA2(DATABASE(),256),48)),0)";

#[derive(Debug, Error)]
pub(super) enum RepositoryError {
    #[error("another cleanup instance holds the database lock")]
    Busy,
    #[error("the server could not obtain a cleanup lock")]
    LockUnavailable,
    #[error("unsupported hMailServer schema: {0}")]
    Schema(String),
    #[error("snapshot must be loaded before selecting messages")]
    NoSnapshot,
    #[error("message batch changed since it was selected")]
    StaleBatch,
    #[error("invalid message batch: {0}")]
    InvalidBatch(&'static str),
    #[error("confirmed deletion still has related rows")]
    Verification,
    #[error("deletion failed: {original}; rollback also failed: {rollback}")]
    Rollback {
        original: String,
        rollback: sqlx::Error,
    },
}

pub struct MySqlRepository {
    connection: MySqlConnection,
    version: String,
    registry: Option<Registry>,
    duplicate_names: HashSet<String>,
}

impl MySqlRepository {
    pub async fn connect(config: &DatabaseConfig) -> PortResult<Self> {
        let mut connection = MySqlConnection::connect_with(&config.options).await?;
        let lock: Option<i64> = sqlx::query_scalar(LOCK_QUERY)
            .fetch_one(&mut connection)
            .await?;
        match lock {
            Some(1) => {}
            Some(0) => return Err(RepositoryError::Busy.into()),
            Some(_) | None => return Err(RepositoryError::LockUnavailable.into()),
        }
        schema::verify(&mut connection).await?;
        let version = sqlx::query_scalar("SELECT VERSION()")
            .fetch_one(&mut connection)
            .await?;
        Ok(Self {
            connection,
            version,
            registry: None,
            duplicate_names: HashSet::new(),
        })
    }

    pub fn server_version(&self) -> &str {
        &self.version
    }
    /// Check SELECT/DELETE permissions without changing any row, then rollback.
    /// Destructive composition points call this before the file step; preview does not.
    pub async fn verify_delete_access(&mut self) -> PortResult<()> {
        delete::verify_access(&mut self.connection).await
    }

    pub async fn close(self) -> PortResult<()> {
        self.connection.close().await?;
        Ok(())
    }
}

impl Repository for MySqlRepository {
    async fn snapshot(&mut self) -> PortResult<Snapshot> {
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut self.connection)
            .await?;
        let mut transaction = self.connection.begin().await?;
        let upper_id: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(messageid),0) FROM hm_messages")
                .fetch_one(&mut *transaction)
                .await?;
        let registry = registry::load(&mut transaction).await?;
        let duplicate_names = duplicates::load(&mut transaction).await?;
        transaction.commit().await?;
        self.registry = Some(registry);
        self.duplicate_names = duplicate_names;
        Ok(Snapshot { upper_id })
    }

    async fn next_batch(
        &mut self,
        selection: Selection,
        cursor: i64,
        limit: u32,
    ) -> PortResult<Batch> {
        let registry = self.registry.as_ref().ok_or(RepositoryError::NoSnapshot)?;
        select::next(
            &mut self.connection,
            registry,
            &self.duplicate_names,
            selection,
            cursor,
            limit,
        )
        .await
    }

    async fn delete_batch(&mut self, candidates: &[Candidate]) -> PortResult<DeleteResult> {
        delete::batch(&mut self.connection, candidates).await
    }

    async fn verify_deleted(&mut self, ids: &[i64]) -> PortResult<()> {
        delete::verify(&mut self.connection, ids).await
    }
}
