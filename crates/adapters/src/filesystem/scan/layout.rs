use super::super::validation;
use hmail_cleaner_core::{
    maintenance::FileObservation,
    paths::{message_guid, validate_component},
};
use std::{fs, io, path::Path};

#[derive(Debug, Clone, Copy)]
pub(super) enum Level {
    Root,
    Domain,
    Account,
    Bucket,
}
pub(super) fn child_directory(level: Level, name: &str, metadata: &fs::Metadata) -> Option<Level> {
    if validation::linked(metadata) || !metadata.is_dir() || validate_component(name).is_err() {
        return None;
    }
    match level {
        Level::Root if !name.eq_ignore_ascii_case("#Public") => Some(Level::Domain),
        Level::Domain => Some(Level::Account),
        Level::Account if name.len() == 2 && name.bytes().all(|byte| byte.is_ascii_hexdigit()) => {
            Some(Level::Bucket)
        }
        Level::Root | Level::Account | Level::Bucket => None,
    }
}
pub(in crate::filesystem) fn observation(
    path: &Path,
    metadata: &fs::Metadata,
) -> io::Result<Option<FileObservation>> {
    if validation::linked(metadata) || !metadata.is_file() {
        return Ok(None);
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(None);
    };
    if validate_component(name).is_err() {
        return Ok(None);
    }
    let Ok(guid) = message_guid(name) else {
        return Ok(None);
    };
    let Some(bucket) = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
    else {
        return Ok(None);
    };
    if !name[1..3].eq_ignore_ascii_case(bucket) {
        return Ok(None);
    }
    Ok(Some(FileObservation {
        path: path.to_path_buf(),
        guid,
        size_bytes: metadata.len(),
        modified_at: metadata.modified()?,
    }))
}
pub(in crate::filesystem) fn allowed_path(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let parts = relative.components().collect::<Vec<_>>();
    if parts.len() != 4 {
        return false;
    }
    let Some(domain) = parts[0].as_os_str().to_str() else {
        return false;
    };
    let Some(account) = parts[1].as_os_str().to_str() else {
        return false;
    };
    let Some(bucket) = parts[2].as_os_str().to_str() else {
        return false;
    };
    validate_component(domain).is_ok()
        && !domain.eq_ignore_ascii_case("#Public")
        && validate_component(account).is_ok()
        && bucket.len() == 2
        && bucket.bytes().all(|byte| byte.is_ascii_hexdigit())
}
