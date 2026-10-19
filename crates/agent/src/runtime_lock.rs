use crate::execution::AgentError;
use std::{fs::File, path::Path};

pub(super) fn acquire(directory: &Path) -> Result<File, AgentError> {
    let path = directory.join("agent.lock");
    if std::fs::symlink_metadata(&path)
        .is_ok_and(|metadata| !metadata.is_file() || metadata.file_type().is_symlink())
    {
        return Err(AgentError::Invalid);
    }
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    lock.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => AgentError::Busy,
        std::fs::TryLockError::Error(error) => AgentError::Transport(error),
    })?;
    Ok(lock)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_one_process_may_recover_or_execute_a_ledger() {
        let directory = tempfile::tempdir().unwrap();
        let owner = acquire(directory.path()).unwrap();
        assert!(matches!(acquire(directory.path()), Err(AgentError::Busy)));
        drop(owner);
        assert!(acquire(directory.path()).is_ok());
    }
}
