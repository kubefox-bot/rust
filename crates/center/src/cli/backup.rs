use crate::{
    config::CenterConfig,
    storage::{BackupManifest, CenterStore, DataDirectory},
};
use std::path::Path;

pub(super) async fn create(
    config: &CenterConfig,
    destination: &Path,
    build_version: &str,
) -> Result<BackupManifest, Box<dyn std::error::Error + Send + Sync>> {
    if !config.data_dir.join("center.sqlite").try_exists()? {
        return Err("Center is not initialized".into());
    }
    let directory = DataDirectory::acquire(&config.data_dir)?;
    let store = CenterStore::open(&directory).await?;
    if store.account().await?.is_none() {
        return Err("Center is not initialized".into());
    }
    Ok(store.backup_with_build(destination, build_version).await?)
}
