mod absolute;

use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

use crate::cleanup::Candidate;
use absolute::Absolute;

const MESSAGE_EXTENSION: &str = ".eml";
const BRACED_GUID_LENGTH: usize = 38;
const WINDOWS_INVALID_CHARACTERS: &[char] = &['<', '>', '"', ':', '|', '?', '*', '/', '\\'];
const WINDOWS_DEVICE_NAMES: &[&str] = &["CON", "PRN", "AUX", "NUL"];

#[derive(Debug, Error)]
pub enum PathError {
    #[error("Data directory must be an absolute, valid storage path")]
    InvalidRoot,
    #[error("message path is outside Data or has an unsupported relative form")]
    OutsideRoot,
    #[error("message filename must be a braced GUID with .eml extension")]
    InvalidFilename,
    #[error("account address does not identify a valid storage directory")]
    InvalidAddress,
    #[error("storage component contains unsupported characters or a reserved name")]
    InvalidComponent,
}

pub fn resolve_path(root: &Path, candidate: &Candidate) -> Result<PathBuf, PathError> {
    let root_text = root.to_str().ok_or(PathError::InvalidRoot)?;
    let parsed_root = Absolute::parse(root_text).map_err(|_| PathError::InvalidRoot)?;
    let filename = candidate.filename.as_str();

    if filename.contains(['/', '\\', ':']) {
        let absolute = Absolute::parse(filename).map_err(|_| PathError::OutsideRoot)?;
        let suffix = absolute
            .suffix_in(&parsed_root)
            .ok_or(PathError::OutsideRoot)?;
        let basename = suffix.last().copied().ok_or(PathError::InvalidFilename)?;
        guid_bucket(basename)?;
        let mut resolved = root.to_path_buf();
        resolved.extend(suffix);
        return Ok(resolved);
    }

    let (account, domain) = candidate
        .account_address
        .rsplit_once('@')
        .ok_or(PathError::InvalidAddress)?;
    validate_component(account).map_err(|_| PathError::InvalidAddress)?;
    validate_component(domain).map_err(|_| PathError::InvalidAddress)?;
    if account.contains('@') || domain.contains('@') {
        return Err(PathError::InvalidAddress);
    }
    let bucket = guid_bucket(filename)?;
    let mut resolved = root.to_path_buf();
    resolved.extend([domain, account, bucket, filename]);
    Ok(resolved)
}

pub fn message_guid(filename: &str) -> Result<Uuid, PathError> {
    let basename = filename
        .rsplit(['/', '\\'])
        .next()
        .ok_or(PathError::InvalidFilename)?;
    parse_guid(basename)
}

fn guid_bucket(filename: &str) -> Result<&str, PathError> {
    parse_guid(filename)?;
    filename.get(1..3).ok_or(PathError::InvalidFilename)
}

fn parse_guid(filename: &str) -> Result<Uuid, PathError> {
    let extension_start = filename
        .len()
        .checked_sub(MESSAGE_EXTENSION.len())
        .ok_or(PathError::InvalidFilename)?;
    let extension = filename
        .get(extension_start..)
        .ok_or(PathError::InvalidFilename)?;
    if !extension.eq_ignore_ascii_case(MESSAGE_EXTENSION) {
        return Err(PathError::InvalidFilename);
    }
    let guid = filename
        .get(..extension_start)
        .ok_or(PathError::InvalidFilename)?;
    if guid.len() != BRACED_GUID_LENGTH || !guid.starts_with('{') || !guid.ends_with('}') {
        return Err(PathError::InvalidFilename);
    }
    Uuid::parse_str(guid).map_err(|_| PathError::InvalidFilename)
}

pub fn validate_component(component: &str) -> Result<(), PathError> {
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.ends_with('.')
        || component.ends_with(' ')
        || component
            .chars()
            .any(|value| value.is_control() || WINDOWS_INVALID_CHARACTERS.contains(&value))
    {
        return Err(PathError::InvalidComponent);
    }
    let stem = component.split('.').next().unwrap_or(component);
    let uppercase = stem.to_ascii_uppercase();
    let device_number = uppercase
        .strip_prefix("COM")
        .or_else(|| uppercase.strip_prefix("LPT"));
    if WINDOWS_DEVICE_NAMES.contains(&uppercase.as_str())
        || device_number
            .is_some_and(|value| value.len() == 1 && matches!(value.as_bytes()[0], b'1'..=b'9'))
    {
        return Err(PathError::InvalidComponent);
    }
    Ok(())
}
