use super::InstallError;
use std::{fs::File, path::Path};
const LOCK_FILE: &str = "installation.lock";
pub(crate) fn acquire(directory: &Path) -> Result<File, InstallError> {
    let path = directory.join(LOCK_FILE);
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| !metadata.is_file() || metadata.file_type().is_symlink())
    {
        return Err(InstallError::Invalid);
    }
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    lock.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => InstallError::Busy,
        std::fs::TryLockError::Error(error) => InstallError::Io(error),
    })?;
    Ok(lock)
}
