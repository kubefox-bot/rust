use crate::{config::private_read, Error};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::os::unix::fs::OpenOptionsExt;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;
pub const JOURNAL_LIMIT: u64 = 1024 * 1024;
pub fn load<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, Error> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Storage),
    }
    serde_json::from_slice(&private_read(path, JOURNAL_LIMIT)?)
        .map(Some)
        .map_err(|_| Error::Invalid)
}
pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<(), Error> {
    let parent = path.parent().ok_or(Error::Invalid)?;
    let temporary = parent.join(format!(".{}.new", Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| Error::Storage)?;
        let bytes = serde_json::to_vec(value).map_err(|_| Error::Invalid)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| Error::Storage)?;
        std::fs::rename(&temporary, path).map_err(|_| Error::Storage)?;
        std::fs::File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::Storage)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Staged,
    Draining,
    Stopped,
    BackedUp,
    Starting,
    Verified,
    Exposing,
    Committed,
    Restoring,
    RollbackExposing,
    PreviousExposing,
    Failed,
    Quarantined,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transaction {
    pub version: u8,
    pub id: Uuid,
    pub phase: Phase,
    pub previous_image: String,
    pub previous_build: String,
    pub candidate_image: String,
    pub candidate_build: String,
    pub sequence: u64,
    pub backup: PathBuf,
    pub candidate_export: PathBuf,
    pub previous_unit: PathBuf,
}
impl Transaction {
    pub fn can_restore(&self) -> bool {
        matches!(
            self.phase,
            Phase::Stopped | Phase::BackedUp | Phase::Starting | Phase::Verified | Phase::Restoring
        )
    }
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub installed_sequence: u64,
    pub installed_build: Option<String>,
    pub installed_digest: Option<String>,
    pub last_check: Option<u64>,
    pub desired_build: Option<String>,
    pub quarantined_digest: Option<String>,
    pub error: Option<String>,
}
pub struct OperationLock(std::fs::File);
impl OperationLock {
    pub fn acquire(path: &Path) -> Result<Self, Error> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| Error::Storage)?;
        file.try_lock().map_err(|_| Error::Conflict)?;
        Ok(Self(file))
    }
}
impl Drop for OperationLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
