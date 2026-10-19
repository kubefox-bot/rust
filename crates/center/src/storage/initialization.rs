use super::{DataDirectory, StorageError, SCHEMA_VERSION};
use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection};
use std::path::Path;

/// Observes an existing initialization without creating or migrating its database.
pub(crate) async fn initialized(path: &Path) -> Result<bool, StorageError> {
    let lease = DataDirectory::acquire(path)?;
    let database = lease.path().join("center.sqlite");
    let metadata = match database.symlink_metadata() {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(StorageError::Database),
    };
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
    let schema: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut connection)
        .await
        .map_err(|_| StorageError::Database)?;
    if integrity != "ok" || schema != SCHEMA_VERSION {
        return Err(StorageError::Schema);
    }
    let complete: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM account WHERE singleton=1 AND revision>0 AND length(material)>0)
         AND EXISTS(SELECT 1 FROM identity WHERE singleton=1 AND length(vault_id)=36)
         AND (SELECT COUNT(*) FROM encrypted_records WHERE kind IN ('ca_key','center_client_key'))=2"
    ).fetch_one(&mut connection).await.map_err(|_| StorageError::Database)?;
    connection
        .close()
        .await
        .map_err(|_| StorageError::Database)?;
    if complete == 0 {
        return Ok(false);
    }
    for name in ["ca-cert.pem", "https-cert.pem", "https-key.pem"] {
        let metadata = lease
            .path()
            .join(name)
            .symlink_metadata()
            .map_err(|_| StorageError::Database)?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(StorageError::Database);
        }
    }
    Ok(true)
}
