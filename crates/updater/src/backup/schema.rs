use crate::Error;
use std::{io::Read, os::unix::fs::OpenOptionsExt, path::Path};
const SQLITE_HEADER_BYTES: usize = 100;
const SQLITE_VERSION_OFFSET: usize = 60;
const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";
pub(super) fn read(path: &Path) -> Result<i64, Error> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| Error::Storage)?;
    if !file.metadata().map_err(|_| Error::Storage)?.is_file() {
        return Err(Error::Verification);
    }
    let mut header = [0; SQLITE_HEADER_BYTES];
    file.read_exact(&mut header)
        .map_err(|_| Error::Verification)?;
    if &header[..SQLITE_MAGIC.len()] != SQLITE_MAGIC {
        return Err(Error::Verification);
    }
    let bytes = header[SQLITE_VERSION_OFFSET..SQLITE_VERSION_OFFSET + 4]
        .try_into()
        .map_err(|_| Error::Verification)?;
    Ok(i64::from(u32::from_be_bytes(bytes)))
}
