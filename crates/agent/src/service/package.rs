use super::{InstallError, InstallationReceipt};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};
const CHUNK_BYTES: usize = 64 * 1024;

pub(super) fn digest(path: &Path) -> Result<[u8; 32], InstallError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(InstallError::Invalid);
    }
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; CHUNK_BYTES];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize().into())
}
pub(super) fn publish(
    source: &Path,
    target: &Path,
    expected: [u8; 32],
    prior: Option<&InstallationReceipt>,
) -> Result<Option<String>, InstallError> {
    let current = validate(target, prior)?;
    if current == Some(expected) {
        return Ok(prior.and_then(|receipt| receipt.previous_sha256.clone()));
    }
    let staged = target.with_extension("exe.new");
    let previous = target.with_extension("exe.previous");
    if staged.exists() {
        return Err(InstallError::Invalid);
    }
    let mut output = File::options().write(true).create_new(true).open(&staged)?;
    let result = (|| {
        std::io::copy(&mut File::open(source)?, &mut output)?;
        output.flush()?;
        output.sync_all()?;
        if digest(&staged)? != expected {
            return Err(InstallError::Invalid);
        }
        Ok(())
    })();
    drop(output);
    if let Err(error) = result {
        let _ = fs::remove_file(&staged);
        return Err(error);
    }
    if previous.exists() {
        fs::remove_file(&previous)?;
    }
    if current.is_some() {
        if let Err(error) = fs::rename(target, &previous) {
            let _ = fs::remove_file(&staged);
            return Err(error.into());
        }
    }
    if let Err(error) = fs::rename(&staged, target) {
        if current.is_some() {
            fs::rename(&previous, target)?;
        }
        return Err(error.into());
    }
    Ok(current.map(|digest| hex(&digest)))
}
/// Validate both files against the protected installation record before stopping SCM.
pub(super) fn validate(
    target: &Path,
    prior: Option<&InstallationReceipt>,
) -> Result<Option<[u8; 32]>, InstallError> {
    let current = optional_digest(target)?;
    let previous = optional_digest(&target.with_extension("exe.previous"))?;
    match (current, prior) {
        (None, None) if previous.is_none() => Ok(None),
        (Some(digest), Some(receipt))
            if hex(&digest) == receipt.sha256
                && previous.map(|digest| hex(&digest)) == receipt.previous_sha256 =>
        {
            Ok(Some(digest))
        }
        _ => Err(InstallError::Invalid),
    }
}
fn optional_digest(path: &Path) -> Result<Option<[u8; 32]>, InstallError> {
    match fs::symlink_metadata(path) {
        Ok(_) => digest(path).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
