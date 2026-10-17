use super::InstallError;
use std::path::Path;
/// Freeze admission durably before an authenticated OS operator replaces expired public trust.
/// The normal installer is responsible for ending maintenance and restarting the agent.
pub async fn prepare_recovery(directory: &Path) -> Result<(), InstallError> {
    if !directory.is_absolute() {
        return Err(InstallError::Invalid);
    }
    super::security::directory(directory)?;
    let metadata = std::fs::symlink_metadata(directory.join("jobs.sqlite"))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(InstallError::Invalid);
    }
    let _lock = super::acquire_update_lock(directory)?;
    let repository = crate::jobs::JobRepository::open(&directory.join("jobs.sqlite")).await?;
    let ready = repository.begin_maintenance().await;
    repository.close().await;
    if !ready? {
        return Err(InstallError::Busy);
    }
    Ok(())
}
