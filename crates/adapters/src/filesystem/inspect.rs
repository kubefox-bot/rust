use super::validation;
use hmail_cleaner_core::cleanup::{
    FileFailure, FileInspection, FileMetadata, FileTarget, PortResult,
};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

pub(super) fn metadata(path: &Path) -> Result<Option<FileMetadata>, FileFailure> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(validation::io_failure(error)),
    };
    if validation::linked(&metadata) || !metadata.is_file() {
        return Err(validation::invalid(
            "message is a link/reparse point or not a regular file",
        ));
    }
    Ok(Some(FileMetadata {
        created_at: metadata.created().map_err(validation::io_failure)?,
        modified_at: metadata.modified().map_err(validation::io_failure)?,
        size_bytes: metadata.len(),
    }))
}

pub(super) async fn run(
    root: PathBuf,
    files: Vec<FileTarget>,
    workers: u8,
) -> PortResult<Vec<FileInspection>> {
    let mut groups: BTreeMap<PathBuf, Vec<FileTarget>> = BTreeMap::new();
    for file in files {
        groups
            .entry(file.path.parent().unwrap_or(&file.path).to_path_buf())
            .or_default()
            .push(file);
    }
    super::workers::run(
        groups.into_values().collect(),
        usize::from(workers),
        move |group| {
            let parent = group[0].path.parent().unwrap_or(&group[0].path);
            let valid = validation::chain(&root, parent);
            group
                .into_iter()
                .map(|file| FileInspection {
                    result: match &valid {
                        Err(error) => Err(error.clone()),
                        Ok(false) => Ok(None),
                        Ok(true) => metadata(&file.path),
                    },
                    id: file.id,
                    path: file.path,
                })
                .collect()
        },
        |file, error| FileInspection {
            id: file.id,
            path: file.path,
            result: Err(validation::invalid(&error)),
        },
    )
    .await
}
