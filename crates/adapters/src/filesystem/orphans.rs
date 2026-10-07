use super::{scan, validation};
use hmail_cleaner_core::{
    cleanup::{DeletionResult, PortResult},
    maintenance::{FileObservation, OrphanFileOutcome},
};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

pub(super) async fn run(
    root: PathBuf,
    files: Vec<FileObservation>,
    workers: u8,
) -> PortResult<Vec<OrphanFileOutcome>> {
    let mut groups: BTreeMap<PathBuf, Vec<FileObservation>> = BTreeMap::new();
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
            group
                .into_iter()
                .map(|observation| {
                    let result = remove(&root, &observation);
                    OrphanFileOutcome {
                        observation,
                        result,
                    }
                })
                .collect()
        },
        |observation, message| OrphanFileOutcome {
            observation,
            result: DeletionResult::Failed(validation::invalid(&message)),
        },
    )
    .await
}
fn remove(root: &Path, observed: &FileObservation) -> DeletionResult {
    if !scan::allowed_path(root, &observed.path) {
        return DeletionResult::Failed(validation::invalid("invalid orphan file layout"));
    }
    let parent = observed.path.parent().expect("validated file layout");
    match validation::chain(root, parent) {
        Err(error) => return DeletionResult::Failed(error),
        Ok(false) => return DeletionResult::AlreadyAbsent,
        Ok(true) => {}
    }
    let metadata = match fs::symlink_metadata(&observed.path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return DeletionResult::AlreadyAbsent
        }
        Err(error) => return DeletionResult::Failed(validation::io_failure(error)),
    };
    match scan::observation(&observed.path, &metadata) {
        Ok(Some(current)) if &current == observed => super::removal::remove(&observed.path),
        Ok(_) => DeletionResult::Failed(validation::invalid("file changed since scan")),
        Err(error) => DeletionResult::Failed(validation::io_failure(error)),
    }
}
