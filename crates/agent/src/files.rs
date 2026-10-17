use std::{
    fs::{self, File},
    io::{self, Write},
    path::Path,
};
/// Replace one application-owned file in a protected directory, preserving an incomplete stage on failure.
pub(crate) fn replace(directory: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    let target = directory.join(name);
    match target.symlink_metadata() {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            return Err(io::Error::from(io::ErrorKind::InvalidInput))
        }
        Ok(_) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    let staged = directory.join(format!("{name}.new"));
    let mut options = File::options();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&staged)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    rename(&staged, &target)?;
    #[cfg(unix)]
    File::open(directory)?.sync_all()?;
    Ok(())
}
#[cfg(not(windows))]
fn rename(source: &Path, target: &Path) -> io::Result<()> {
    fs::rename(source, target)
}
#[cfg(windows)]
fn rename(source: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // Paths are application-owned, the source file has been flushed and closed.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
pub(crate) fn read(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    use std::io::Read;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > limit {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    Ok(bytes)
}
