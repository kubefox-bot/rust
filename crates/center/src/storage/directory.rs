use super::StorageError;
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::Arc,
};
pub struct DataDirectory;
pub struct DataDirectoryLease(pub(super) Arc<Lease>);
pub(super) struct Lease {
    pub path: PathBuf,
    _lock: File,
}
impl DataDirectory {
    pub fn acquire(path: &Path) -> Result<DataDirectoryLease, StorageError> {
        std::fs::create_dir_all(path).map_err(|_| StorageError::Directory)?;
        if std::fs::symlink_metadata(path)
            .map_err(|_| StorageError::Directory)?
            .file_type()
            .is_symlink()
        {
            return Err(StorageError::Directory);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| StorageError::Directory)?;
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
