use super::{connection::SCHEMA_VERSION, CenterStore, StorageError};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection};
use std::path::Path;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupManifest {
    pub format_version: u16,
    pub schema_version: i64,
    pub vault_id: Option<uuid::Uuid>,
    pub build_version: String,
}
impl CenterStore {
    pub async fn backup(&self, destination: &Path) -> Result<BackupManifest, StorageError> {
        self.backup_with_build(destination, env!("CARGO_PKG_VERSION"))
            .await
    }
    pub async fn backup_with_build(
        &self,
        destination: &Path,
        build_version: &str,
    ) -> Result<BackupManifest, StorageError> {
        crate::config::validate_build_version(build_version).map_err(|_| StorageError::Backup)?;
        std::fs::create_dir(destination).map_err(|_| StorageError::Backup)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(destination, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| StorageError::Backup)?;
        }
        let database = destination.join("center.sqlite");
        let path = database.to_str().ok_or(StorageError::Backup)?;
        let options = SqliteConnectOptions::new().filename(self.lease.path.join("center.sqlite"));
        let mut connection = SqliteConnection::connect_with(&options)
            .await
            .map_err(|_| StorageError::Backup)?;
        sqlx::query("VACUUM INTO ?")
            .bind(path)
            .execute(&mut connection)
            .await
            .map_err(|_| StorageError::Backup)?;
        connection.close().await.map_err(|_| StorageError::Backup)?;
        let mut snapshot = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&database)
                .read_only(true),
        )
        .await
        .map_err(|_| StorageError::Backup)?;
        let vault_id: Option<String> =
            sqlx::query_scalar("SELECT vault_id FROM identity WHERE singleton=1")
                .fetch_optional(&mut snapshot)
                .await
                .map_err(|_| StorageError::Backup)?;
        snapshot.close().await.map_err(|_| StorageError::Backup)?;
        for filename in [
            "https-cert.pem",
            "https-key.pem",
            "ca-cert.pem",
            "center.json",
            "update-policy.json",
        ] {
            let source = self.lease.path.join(filename);
            match source.symlink_metadata() {
                Ok(meta) if meta.is_file() && meta.len() <= 65536 => {
                    std::fs::copy(source, destination.join(filename))
                        .map_err(|_| StorageError::Backup)?;
                }
                Ok(_) => return Err(StorageError::Backup),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(_) => return Err(StorageError::Backup),
            }
        }
        let manifest = BackupManifest {
            format_version: 1,
            schema_version: SCHEMA_VERSION,
            vault_id: vault_id
                .map(|id| id.parse())
                .transpose()
                .map_err(|_| StorageError::Backup)?,
            build_version: build_version.into(),
        };
        let bytes = serde_json::to_vec_pretty(&manifest).map_err(|_| StorageError::Backup)?;
        std::fs::write(destination.join("manifest.json"), bytes)
            .map_err(|_| StorageError::Backup)?;
        for entry in std::fs::read_dir(destination).map_err(|_| StorageError::Backup)? {
            let path = entry.map_err(|_| StorageError::Backup)?.path();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|_| StorageError::Backup)?;
            }
            std::fs::File::open(path)
                .and_then(|file| file.sync_all())
                .map_err(|_| StorageError::Backup)?;
        }
        #[cfg(unix)]
        std::fs::File::open(destination)
            .and_then(|file| file.sync_all())
            .map_err(|_| StorageError::Backup)?;
        Ok(manifest)
    }
}
