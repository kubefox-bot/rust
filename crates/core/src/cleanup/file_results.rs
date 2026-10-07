use super::{
    batch::{self, Resolved},
    *,
};
use std::{collections::HashSet, path::PathBuf};
pub(super) fn accept<P: Fn(Progress)>(
    mut planned: Resolved,
    outcomes: Vec<FileOutcome>,
    summary: &mut Summary,
    progress: &P,
) -> PortResult<(Vec<Candidate>, Vec<PathBuf>)> {
    if planned.len() != outcomes.len() {
        return Err("file store returned an incomplete batch".into());
    }
    let mut seen = HashSet::new();
    for outcome in &outcomes {
        if !seen.insert(outcome.id)
            || !planned
                .get(&outcome.id)
                .is_some_and(|(_, path)| *path == outcome.path)
        {
            return Err("file store returned an unexpected ID or path".into());
        }
    }
    let mut eligible = Vec::new();
    let mut parents = Vec::new();
    for outcome in outcomes {
        let (candidate, path) = planned.remove(&outcome.id).expect("validated file outcome");
        match outcome.result {
            DeletionResult::Removed => summary.removed_files += 1,
            DeletionResult::AlreadyAbsent => summary.absent_files += 1,
            DeletionResult::Failed(error) => {
                batch::failure(
                    summary,
                    Stage::Files,
                    Failure::Message {
                        id: outcome.id,
                        reason: error.message,
                    },
                    progress,
                );
                continue;
            }
        }
        eligible.push(candidate);
        if let Some(parent) = path.parent() {
            parents.push(parent.to_path_buf());
        }
    }
    eligible.sort_by_key(|row| row.id);
    Ok((eligible, parents))
}
