use super::validation;
use hmail_cleaner_core::cleanup::{DirectoryFailure, PruneOutcome};
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

pub(super) fn run(root: &Path, parents: Vec<PathBuf>) -> PruneOutcome {
    let mut parents = parents
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    parents.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    let mut outcome = PruneOutcome::default();
    for parent in parents {
        if let Err(error) = validation::chain(root, &parent) {
            outcome.failures.push(DirectoryFailure {
                path: parent,
                reason: error.message,
            });
            continue;
        }
        let mut current = parent;
        while current != root {
            match fs::remove_dir(&current) {
                Ok(()) => outcome.removed += 1,
                Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => break,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    outcome.failures.push(DirectoryFailure {
                        path: current,
                        reason: error.to_string(),
                    });
                    break;
                }
            }
            let Some(parent) = current.parent() else {
                break;
            };
            current = parent.to_path_buf();
        }
    }
    outcome
}

/// The recursive walker still owns ancestor directory handles. Remove only fully visited directories.
pub(super) fn completed(root: &Path, paths: Vec<PathBuf>) -> PruneOutcome {
    let mut outcome = PruneOutcome::default();
    for path in paths {
        if path == root {
            continue;
        }
        match validation::chain(root, &path) {
            Ok(false) => continue,
            Err(error) => {
                outcome.failures.push(DirectoryFailure {
                    path,
                    reason: error.message,
                });
                continue;
            }
            Ok(true) => {}
        }
        match fs::remove_dir(&path) {
            Ok(()) => outcome.removed += 1,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::DirectoryNotEmpty | io::ErrorKind::NotFound
                ) => {}
            Err(error) => outcome.failures.push(DirectoryFailure {
                path,
                reason: error.to_string(),
            }),
        }
    }
    outcome
}
