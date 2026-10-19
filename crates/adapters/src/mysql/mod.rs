mod config;
mod delete;
mod duplicates;
mod ids;
mod inventory;
mod orphans;
mod references;
mod registry;
mod schema;
mod select;
mod storage;
mod transaction;
mod tree;

pub use config::{DatabaseConfig, DatabaseConfigError};

use hmail_cleaner_core::cleanup::{
    Batch, Candidate, DeleteResult, PortResult, Repository, Selection, Snapshot,
};
use registry::Registry;
use sqlx::{Connection, MySqlConnection};
use std::collections::HashSet;
use thiserror::Error;

const DELIVERED_STATE: i8 = 2;
// Created, delivering, and held queue messages all use root storage for bare names.
const QUEUE_STATES: &[i8] = &[0, 1, 3];
pub(super) const MAX_BATCH_SIZE: usize = hmail_cleaner_core::cleanup::MAX_BATCH_SIZE as usize;
const LOCK_QUERY: &str = "SELECT GET_LOCK(CONCAT('hmail-clean:', LEFT(SHA2(DATABASE(),256),48)),0)";

#[derive(Debug, Error)]
pub(super) enum RepositoryError {
    #[error("another cleanup instance holds the database lock")]
    Busy,
    #[error("Некорректный путь записи БД {id}: {reason}")]
    InvalidStoredPath { id: i64, reason: String },
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
    inventory: Option<inventory::Index>,
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
            inventory: None,
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
        let index = match self.inventory {
            Some(index) => index.close().await,
            None => Ok(()),
        };
        let mysql = self.connection.close().await;
        match (index, mysql) {
            (Err(index), Err(mysql)) => {
                Err(format!("Закрытие индекса: {index}; закрытие MySQL: {mysql}").into())
            }
            (Err(error), _) => Err(error),
            (Ok(()), result) => result.map_err(Into::into),
        }
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

impl hmail_cleaner_core::maintenance::FullRepository for MySqlRepository {
    async fn orphan_upper_id(
        &mut self,
        table: hmail_cleaner_core::maintenance::OrphanTable,
    ) -> PortResult<i64> {
        orphans::selection::upper(&mut self.connection, table).await
    }
    async fn orphan_batch(
        &mut self,
        selection: hmail_cleaner_core::maintenance::OrphanSelection,
        cursor: i64,
        limit: u32,
    ) -> PortResult<hmail_cleaner_core::maintenance::OrphanBatch> {
        orphans::selection::next(&mut self.connection, selection, cursor, limit).await
    }
    async fn delete_orphans(
        &mut self,
        table: hmail_cleaner_core::maintenance::OrphanTable,
        rows: &[hmail_cleaner_core::maintenance::OrphanRow],
    ) -> PortResult<DeleteResult> {
        orphans::deletion::delete(&mut self.connection, table, rows).await
    }
    async fn verify_orphans_deleted(
        &mut self,
        table: hmail_cleaner_core::maintenance::OrphanTable,
        ids: &[i64],
    ) -> PortResult<()> {
        orphans::deletion::verify(&mut self.connection, table, ids).await
    }
    async fn file_references(
        &mut self,
        limit: usize,
    ) -> PortResult<hmail_cleaner_core::maintenance::FileReferences> {
        references::load(&mut self.connection, limit).await
    }
}
