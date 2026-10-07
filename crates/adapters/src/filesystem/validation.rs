use hmail_cleaner_core::cleanup::{FileFailure, FileFailureKind};
use std::{
    fs, io,
    path::{Component, Path},
};

pub(super) fn io_failure(error: io::Error) -> FileFailure {
    FileFailure {
        kind: match error.kind() {
            io::ErrorKind::PermissionDenied => FileFailureKind::AccessDenied,
            _ => FileFailureKind::Io,
        },
        message: error.to_string(),
    }
}
pub(super) fn invalid(message: &str) -> FileFailure {
    FileFailure {
        kind: FileFailureKind::InvalidPath,
        message: message.into(),
    }
}
pub(super) fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
pub(super) fn directory(path: &Path) -> Result<(), FileFailure> {
    let metadata = fs::symlink_metadata(path).map_err(io_failure)?;
    if linked(&metadata) || !metadata.is_dir() {
        return Err(invalid(
            "directory is a link/reparse point or not a directory",
        ));
    }
    Ok(())
}
pub(super) fn chain(root: &Path, parent: &Path) -> Result<bool, FileFailure> {
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| invalid("path is outside DataFolder"))?;
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(invalid("invalid path component"));
    }
    directory(root)?;
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if linked(&metadata) || !metadata.is_dir() => {
                return Err(invalid("parent is a link/reparse point or not a directory"))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(io_failure(error)),
        }
    }
    Ok(true)
}

#[cfg(windows)]
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reparse_attributes_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        let link = temp.path().join("link");
        fs::create_dir(&target).unwrap();
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .status()
            .unwrap();
        assert!(status.success(), "could not create test junction");
        assert!(linked(&fs::symlink_metadata(&link).unwrap()));
        assert!(directory(&link).is_err());
    }
}
