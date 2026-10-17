use super::{InstallError, InstallationReceipt};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};
const RECEIPT_NAME: &str = "installation.json";
const RECEIPT_LIMIT: u64 = 16 * 1024;
pub(super) fn read(
    directory: &Path,
    executable: &Path,
) -> Result<Option<InstallationReceipt>, InstallError> {
    let path = directory.join(RECEIPT_NAME);
    let info = match fs::symlink_metadata(&path) {
        Ok(info) => info,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !info.is_file() || info.file_type().is_symlink() {
        return Err(InstallError::Invalid);
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(RECEIPT_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > RECEIPT_LIMIT {
        return Err(InstallError::Invalid);
    }
    let receipt: InstallationReceipt =
        serde_json::from_slice(&bytes).map_err(|_| InstallError::Invalid)?;
    if receipt.executable != executable {
        return Err(InstallError::Collision);
    }
    Ok(Some(receipt))
}
pub(super) fn write(directory: &Path, receipt: &InstallationReceipt) -> Result<(), InstallError> {
    let staged = directory.join("installation.json.new");
    let mut options = File::options();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options.open(&staged)?;
    output.write_all(&serde_json::to_vec(receipt).map_err(|_| InstallError::Invalid)?)?;
    output.sync_all()?;
    drop(output);
    fs::rename(staged, directory.join(RECEIPT_NAME))?;
    Ok(())
}
