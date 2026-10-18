use crate::Error;
use std::os::unix::ffi::OsStrExt;
use std::{ffi::CString, path::Path};
const MIN_HEADROOM: u64 = 256 * 1024 * 1024;
pub fn required(data_bytes: u64) -> Result<u64, Error> {
    data_bytes
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(MIN_HEADROOM))
        .ok_or(Error::Invalid)
}
pub fn tree_bytes(path: &Path) -> Result<u64, Error> {
    fn visit(path: &Path, depth: u8) -> Result<u64, Error> {
        if depth > 64 {
            return Err(Error::Invalid);
        }
        let metadata = std::fs::symlink_metadata(path).map_err(|_| Error::Storage)?;
        if metadata.is_file() {
            return Ok(metadata.len());
        }
        if !metadata.is_dir() {
            return Err(Error::Invalid);
        }
        let mut bytes = 0u64;
        for item in std::fs::read_dir(path).map_err(|_| Error::Storage)? {
            bytes = bytes
                .checked_add(visit(&item.map_err(|_| Error::Storage)?.path(), depth + 1)?)
                .ok_or(Error::Invalid)?;
        }
        Ok(bytes)
    }
    visit(path, 0)
}
pub fn ensure(path: &Path, needed: u64) -> Result<(), Error> {
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::Invalid)?;
    let mut output = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: both pointers are valid for statvfs; initialized output is read only on success.
    let status = unsafe { libc::statvfs(path.as_ptr(), output.as_mut_ptr()) };
    if status != 0 {
        return Err(Error::Storage);
    }
    let values = unsafe { output.assume_init() };
    let available = u128::from(values.f_bavail)
        .checked_mul(u128::from(values.f_frsize))
        .ok_or(Error::Invalid)?;
    if available < u128::from(needed) {
        return Err(Error::Storage);
    }
    Ok(())
}
