use crate::{journal::Transaction, podman, Error};
/// Offline recovery keeps an exported candidate AND a verified old snapshot.
pub async fn run(tx: &Transaction) -> Result<(), Error> {
    super::verify_hashes(tx)?;
    let directory = podman::mountpoint().await?;
    let _lease = crate::journal::OperationLock::acquire(&directory.join("center.lock"))?;
    // Keep the fixed volume and its UID mapping; replace only its offline contents.
    // A crash resumes this same copy from the immutable matching snapshot.
    for entry in std::fs::read_dir(&directory).map_err(|_| Error::Storage)? {
        let path = entry.map_err(|_| Error::Storage)?.path();
        if path.file_name().is_some_and(|name| name == "center.lock") {
            continue;
        }
        let meta = std::fs::symlink_metadata(&path).map_err(|_| Error::Storage)?;
        if meta.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|_| Error::Storage)?;
        } else {
            std::fs::remove_file(&path).map_err(|_| Error::Storage)?;
        }
    }
    for entry in std::fs::read_dir(&tx.backup).map_err(|_| Error::Storage)? {
        let entry = entry.map_err(|_| Error::Storage)?;
        if entry.file_name() == "manifest.json" {
            continue;
        }
        if !entry.file_type().map_err(|_| Error::Storage)?.is_file() {
            return Err(Error::Verification);
        }
        super::filesystem_copy(&entry.path(), &directory.join(entry.file_name())).await?;
    }
    podman::mark(tx.id).await?;
    std::fs::File::open(&directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| Error::Storage)
}
