mod backup;
mod connection;
mod directory;
mod migration;
mod records;

pub use backup::BackupManifest;
pub use connection::CenterStore;
pub(crate) use connection::SCHEMA_VERSION;
pub use directory::{DataDirectory, DataDirectoryLease};
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("center data directory is unavailable or already locked")]
    Directory,
    #[error("center database is invalid or unavailable")]
    Database,
    #[error("unsupported center schema version")]
    Schema,
    #[error("backup destination must be new and writable")]
    Backup,
    #[error("invalid encrypted account record")]
    Account,
}
