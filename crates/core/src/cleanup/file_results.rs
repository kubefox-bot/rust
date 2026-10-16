use super::{
    batch::{self, Resolved},
    *,
};
use std::path::PathBuf;
pub(super) fn accept<P: Fn(Progress)>(
    mut planned: Resolved,
    outcomes: Vec<FileOutcome>,
    summary: &mut Summary,
    progress: &P,
) -> PortResult<(Vec<Candidate>, Vec<PathBuf>)> {
    batch::validate_files(&planned, outcomes.iter().map(|file| (file.id, &file.path)))?;
    let mut eligible = Vec::new();
    let mut parents = Vec::new();
    for outcome in outcomes {
        let (candidate, path) = planned.remove(&outcome.id).expect("validated file outcome");
        match outcome.result {
            DeletionResult::Removed { size_bytes } => {
                summary.removed_files += 1;
                summary.removed_bytes += size_bytes;
            }
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
