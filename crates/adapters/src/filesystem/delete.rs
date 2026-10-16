use super::validation;
use hmail_cleaner_core::cleanup::{DeletionResult, FileInspection, FileOutcome, PortResult};
use std::{collections::BTreeMap, path::PathBuf};

type Group = Vec<FileInspection>;
pub(super) async fn run(
    root: PathBuf,
    files: Vec<FileInspection>,
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
    super::workers::run(
        groups,
        usize::from(workers),
        move |group| remove_group(&root, group),
        |file, message| FileOutcome {
            id: file.id,
            path: file.path,
            result: DeletionResult::Failed(validation::invalid(&message)),
        },
    )
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
                Ok(_) => match (&file.result, super::inspect::metadata(&file.path)) {
                    (Err(error), _) => DeletionResult::Failed(error.clone()),
                    (_, Err(error)) => DeletionResult::Failed(error),
                    (Ok(_), Ok(None)) => DeletionResult::AlreadyAbsent,
                    (Ok(Some(expected)), Ok(Some(current))) if *expected == current => {
                        super::removal::remove(&file.path)
                    }
                    (Ok(_), Ok(Some(_))) => DeletionResult::Failed(validation::invalid(
                        "file changed since age inspection",
                    )),
                },
            };
            FileOutcome {
                id: file.id,
                path: file.path,
                result,
            }
        })
        .collect()
}
