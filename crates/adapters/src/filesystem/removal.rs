use super::{retry, validation};
use hmail_cleaner_core::cleanup::DeletionResult;
use std::{fs, io, path::Path, time::Duration};

trait FileIo {
    fn metadata(&self, path: &Path) -> io::Result<fs::Metadata>;
    fn unlink(&self, path: &Path) -> io::Result<()>;
}
struct Native;
impl FileIo for Native {
    fn metadata(&self, path: &Path) -> io::Result<fs::Metadata> {
        fs::symlink_metadata(path)
    }
    fn unlink(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }
}
#[derive(Clone, Copy)]
enum Phase {
    Delete,
    ConfirmRemoved(u64),
    ConfirmAbsent,
}
enum RemovalError {
    Io(io::Error),
    Pending(io::Error),
    Present,
    Invalid,
}
impl RemovalError {
    fn retryable(&self) -> bool {
        match self {
            Self::Io(error) => cfg!(windows) && retry::windows_lock(error),
            Self::Pending(error) => {
                cfg!(windows) && error.kind() == io::ErrorKind::PermissionDenied
            }
            Self::Present => true,
            Self::Invalid => false,
        }
    }
    fn outcome(self) -> DeletionResult {
        let failure = match self {
            Self::Io(error) | Self::Pending(error) => validation::io_failure(error),
            Self::Present => validation::io_failure(io::Error::other(
                "message still exists after deletion request",
            )),
            Self::Invalid => {
                validation::invalid("message is a link/reparse point or not a regular file")
            }
        };
        DeletionResult::Failed(failure)
    }
}
pub(super) fn remove(path: &Path) -> DeletionResult {
    remove_with(&Native, path, std::thread::sleep)
}
fn remove_with(io: &impl FileIo, path: &Path, pause: impl FnMut(Duration)) -> DeletionResult {
    let mut phase = Phase::Delete;
    let outcome = retry::run(
        || {
            if matches!(phase, Phase::Delete) {
                let metadata = match io.metadata(path) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return Ok(DeletionResult::AlreadyAbsent)
                    }
                    Err(error) => return Err(RemovalError::Io(error)),
                };
                if validation::linked(&metadata) || !metadata.is_file() {
                    return Err(RemovalError::Invalid);
                }
                phase = match io.unlink(path) {
                    Ok(()) => Phase::ConfirmRemoved(metadata.len()),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Phase::ConfirmAbsent,
                    Err(error) => return Err(RemovalError::Io(error)),
                };
            }
            match io.metadata(path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(match phase {
                    Phase::ConfirmRemoved(size_bytes) => DeletionResult::Removed { size_bytes },
                    Phase::ConfirmAbsent => DeletionResult::AlreadyAbsent,
                    Phase::Delete => unreachable!("deletion phase precedes confirmation"),
                }),
                Err(error) => Err(RemovalError::Pending(error)),
                Ok(_) => Err(RemovalError::Present),
            }
        },
        RemovalError::retryable,
        pause,
    );
    outcome.unwrap_or_else(RemovalError::outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmail_cleaner_core::cleanup::DeletionResult;
    use std::{cell::Cell, fs, io, path::Path};

    struct DeniedConfirmation {
        calls: Cell<u32>,
    }
    impl FileIo for DeniedConfirmation {
        fn metadata(&self, path: &Path) -> io::Result<fs::Metadata> {
            let call = self.calls.get();
            self.calls.set(call + 1);
            if call == 0 {
                fs::symlink_metadata(path)
            } else {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
        }
        fn unlink(&self, path: &Path) -> io::Result<()> {
            fs::remove_file(path)
        }
    }
    #[test]
    fn verification_error_is_not_confirmed_removal() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("message.eml");
        fs::write(&path, b"1234567").unwrap();
        let result = remove_with(
            &DeniedConfirmation {
                calls: Cell::new(0),
            },
            &path,
            |_| {},
        );
        assert!(matches!(result, DeletionResult::Failed(_)));
        assert!(!path.try_exists().unwrap());
    }
}
