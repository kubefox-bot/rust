use super::{InstallError, InstallationReceipt};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};
const RECEIPT_NAME: &str = "installation.json";
const RECEIPT_LIMIT: u64 = 16 * 1024;
pub(super) fn read(
    directory: &Path,
    executable: &Path,
) -> Result<Option<InstallationReceipt>, InstallError> {
    let receipt = read_receipt(directory)?;
    if receipt
        .as_ref()
        .is_some_and(|receipt| receipt.executable != executable)
    {
        return Err(InstallError::Collision);
    }
    Ok(receipt)
}
fn read_receipt(directory: &Path) -> Result<Option<InstallationReceipt>, InstallError> {
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
    Ok(Some(receipt))
}
pub(crate) fn status(
    directory: &Path,
    executable: &Path,
) -> Result<Option<hmail_cleaner_protocol::InstallationStatus>, InstallError> {
    let Some(receipt) = read_receipt(directory)? else {
        return Ok(None);
    };
    if std::fs::canonicalize(&receipt.executable)? != std::fs::canonicalize(executable)? {
        return Err(InstallError::Collision);
    }
    if !receipt.started {
        return Ok(None);
    }
    let Some(id) = receipt.operation_id.filter(|id| !id.is_nil()) else {
        return Ok(None);
    };
    if receipt.sha256.len() != 64
        || !receipt
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(InstallError::Invalid);
    }
    Ok(Some(hmail_cleaner_protocol::InstallationStatus {
        operation_id: id,
        sha256: receipt.sha256,
    }))
}
pub(super) fn write(directory: &Path, receipt: &InstallationReceipt) -> Result<(), InstallError> {
    let bytes = serde_json::to_vec(receipt).map_err(|_| InstallError::Invalid)?;
    crate::files::replace(directory, RECEIPT_NAME, &bytes)?;
    Ok(())
}
