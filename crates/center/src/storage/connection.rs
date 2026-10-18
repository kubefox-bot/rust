use super::{directory::Lease, DataDirectoryLease, StorageError};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    Connection, SqliteConnection, SqlitePool,
};
use std::{sync::Arc, time::Duration};
pub(crate) const SCHEMA_VERSION: i64 = 2;
#[derive(Clone)]
pub struct CenterStore {
    pub(crate) pool: SqlitePool,
    maintenance: crate::maintenance::MaintenanceGate,
    pub(super) lease: Arc<Lease>,
    pub(crate) kdf: Arc<crate::vault::KdfGate>,
    pub(crate) catalog_writer: Arc<tokio::sync::Mutex<()>>,
    pub(crate) revoked: hmail_cleaner_adapters::mtls::RevokedSerials,
}
impl CenterStore {
    pub fn maintenance(&self) -> &crate::maintenance::MaintenanceGate {
        &self.maintenance
    }
    pub(crate) fn directory(&self) -> &std::path::Path {
        &self.lease.path
    }
    pub async fn open(directory: &DataDirectoryLease) -> Result<Self, StorageError> {
        let maintenance = crate::maintenance::MaintenanceGate::open(directory.path())
            .map_err(|_| StorageError::Directory)?;
        let database = directory.path().join("center.sqlite");
        if database.exists() {
            let metadata =
                std::fs::symlink_metadata(&database).map_err(|_| StorageError::Database)?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(StorageError::Database);
            }
            let options = SqliteConnectOptions::new()
                .filename(&database)
                .read_only(true);
            let mut connection = SqliteConnection::connect_with(&options)
                .await
                .map_err(|_| StorageError::Database)?;
            let integrity: String = sqlx::query_scalar("PRAGMA quick_check")
                .fetch_one(&mut connection)
                .await
                .map_err(|_| StorageError::Database)?;
            let version: i64 = sqlx::query_scalar("PRAGMA user_version")
                .fetch_one(&mut connection)
                .await
                .map_err(|_| StorageError::Database)?;
            connection
                .close()
                .await
                .map_err(|_| StorageError::Database)?;
            if integrity != "ok" {
                return Err(StorageError::Database);
            }
            if version != 1 && version != SCHEMA_VERSION {
                return Err(StorageError::Schema);
            }
        }
        let is_new = !database.exists();
        let options = SqliteConnectOptions::new()
            .filename(&database)
            .create_if_missing(is_new)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await
            .map_err(|_| StorageError::Database)?;
        if is_new {
            let mut transaction = pool.begin().await.map_err(|_| StorageError::Database)?;
            sqlx::raw_sql(include_str!("../../migrations/0001_center.sql"))
                .execute(&mut *transaction)
                .await
                .map_err(|_| StorageError::Database)?;
            transaction
                .commit()
                .await
                .map_err(|_| StorageError::Database)?;
        }
        super::migration::upgrade(&pool).await?;
        Ok(Self {
            pool,
            maintenance,
            lease: directory.0.clone(),
            kdf: Arc::new(crate::vault::KdfGate::new()),
            catalog_writer: Arc::new(tokio::sync::Mutex::new(())),
            revoked: hmail_cleaner_adapters::mtls::RevokedSerials::default(),
        })
    }
}
