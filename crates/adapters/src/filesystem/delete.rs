use super::validation;
use hmail_cleaner_core::cleanup::{DeletionResult, FileOutcome, FileTarget, PortResult};
use std::{collections::BTreeMap, fs, io, path::PathBuf};

type Group = Vec<FileTarget>;
pub(super) async fn run(
    root: PathBuf,
    files: Vec<FileTarget>,
    workers: u8,
) -> PortResult<Vec<FileOutcome>> {
    let mut groups: BTreeMap<PathBuf, Group> = BTreeMap::new();
    for file in files {
        groups
            .entry(file.path.parent().unwrap_or(&file.path).to_path_buf())
            .or_default()
            .push(file);
    }
    let groups = groups.into_values().collect::<Vec<_>>();
    super::workers::run(groups, usize::from(workers), move |group| {
        remove_group(&root, group)
    })
    .await
}
fn remove_group(root: &std::path::Path, group: Group) -> Vec<FileOutcome> {
    let parent = group[0].path.parent().unwrap_or(&group[0].path);
    let valid = validation::chain(root, parent);
    group
        .into_iter()
        .map(|file| {
            let result = match &valid {
                Err(error) => DeletionResult::Failed(error.clone()),
                Ok(false) => DeletionResult::AlreadyAbsent,
                Ok(true) => remove_one(&file.path),
            };
            FileOutcome {
                id: file.id,
                path: file.path,
                result,
            }
        })
        .collect()
}
fn remove_one(path: &std::path::Path) -> DeletionResult {
    match fs::symlink_metadata(path) {
        Ok(metadata) if validation::linked(&metadata) || !metadata.is_file() => {
            return DeletionResult::Failed(validation::invalid(
                "message is a link/reparse point or not a regular file",
            ))
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return DeletionResult::AlreadyAbsent
        }
        Err(error) => return DeletionResult::Failed(validation::io_failure(error)),
    }
    match fs::remove_file(path) {
        Ok(()) => DeletionResult::Removed,
        Err(error) if error.kind() == io::ErrorKind::NotFound => DeletionResult::AlreadyAbsent,
        Err(error) => DeletionResult::Failed(validation::io_failure(error)),
    }
}
