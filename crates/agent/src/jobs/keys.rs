use super::types::JobStoreError;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
use zeroize::Zeroizing;

const KEY_BYTES: usize = 32;
const KEY_READ_LIMIT: u64 = (KEY_BYTES + 1) as u64;

pub(super) fn load_or_create(database: &Path) -> Result<Zeroizing<[u8; KEY_BYTES]>, JobStoreError> {
    let path = database.with_extension("receipt-key");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            let mut key = Zeroizing::new([0u8; KEY_BYTES]);
            getrandom::fill(&mut *key).map_err(|_| JobStoreError::Random)?;
            file.write_all(&*key)?;
            file.sync_all()?;
            Ok(key)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => load(&path),
        Err(error) => Err(error.into()),
    }
}

fn load(path: &Path) -> Result<Zeroizing<[u8; KEY_BYTES]>, JobStoreError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(JobStoreError::InvalidKey);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(JobStoreError::InvalidKey);
        }
    }
    let mut bytes = Zeroizing::new(Vec::new());
    File::open(path)?
        .take(KEY_READ_LIMIT)
        .read_to_end(&mut bytes)?;
    let key: [u8; KEY_BYTES] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| JobStoreError::InvalidKey)?;
    Ok(Zeroizing::new(key))
}
