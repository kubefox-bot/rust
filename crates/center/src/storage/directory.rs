use super::StorageError;
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::Arc,
};
#[cfg(unix)]
const PRIVATE_MODE: u32 = 0o700;
#[cfg(unix)]
const NON_OWNER_BITS: u32 = 0o077;
pub struct DataDirectory;
pub struct DataDirectoryLease(pub(super) Arc<Lease>);
pub(super) struct Lease {
    pub path: PathBuf,
    _lock: File,
}
impl DataDirectory {
    pub fn acquire(path: &Path) -> Result<DataDirectoryLease, StorageError> {
        std::fs::create_dir_all(path).map_err(|_| StorageError::Directory)?;
        let metadata = std::fs::symlink_metadata(path).map_err(|_| StorageError::Directory)?;
        if metadata.file_type().is_symlink() {
            return Err(StorageError::Directory);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & NON_OWNER_BITS != 0 {
                if std::fs::read_dir(path)
                    .map_err(|_| StorageError::Directory)?
                    .next()
                    .is_some()
                {
                    return Err(StorageError::Directory);
                }
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(PRIVATE_MODE))
                    .map_err(|_| StorageError::Directory)?;
            }
        }
        let lock_path = path.join("center.lock");
        if lock_path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(StorageError::Directory);
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|_| StorageError::Directory)?;
        file.try_lock().map_err(|_| StorageError::Directory)?;
        Ok(DataDirectoryLease(Arc::new(Lease {
            path: path.to_path_buf(),
            _lock: file,
        })))
    }
}
impl DataDirectoryLease {
    pub fn path(&self) -> &Path {
        &self.0.path
    }
}
