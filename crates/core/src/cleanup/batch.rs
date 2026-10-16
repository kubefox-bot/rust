use super::*;
use crate::paths::resolve_path;
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};
pub(super) type Resolved = BTreeMap<i64, (Candidate, PathBuf)>;

pub(super) fn validate_files<'a>(
    planned: &Resolved,
    outcomes: impl IntoIterator<Item = (i64, &'a PathBuf)>,
) -> PortResult<()> {
    let mut seen = HashSet::new();
    for (id, path) in outcomes {
        if !seen.insert(id)
            || !planned
                .get(&id)
                .is_some_and(|(_, expected)| expected == path)
        {
            return Err("file store returned an unexpected ID or path".into());
        }
    }
    if seen.len() != planned.len() {
        return Err("file store returned an incomplete batch".into());
    }
    Ok(())
}
pub(super) fn validate_options(options: &RunOptions) -> PortResult<()> {
    if options.batch_size.get() > MAX_BATCH_SIZE {
        return Err("batch size exceeds 10000".into());
    }
    if !(1..=MAX_WORKERS).contains(&options.workers) {
        return Err("workers must be between 1 and 8".into());
    }
    Ok(())
}
pub(super) fn validate(batch: &Batch, cursor: i64, upper: i64) -> PortResult<bool> {
    if batch.candidates.is_empty() && batch.rejected.is_empty() {
        if batch.last_id != cursor {
            return Err("empty batch moved its cursor".into());
        }
        return Ok(false);
    }
    if batch.last_id <= cursor || batch.last_id > upper {
        return Err("invalid repository cursor".into());
    }
    let mut seen = HashSet::new();
    for id in batch
        .candidates
        .iter()
        .map(|row| row.id)
        .chain(batch.rejected.iter().map(|row| row.id))
    {
        if id <= cursor || id > batch.last_id || !seen.insert(id) {
            return Err("invalid or repeated message ID in batch".into());
        }
    }
    Ok(true)
}
pub(super) fn resolve<P: Fn(Progress)>(
    batch: Batch,
    options: &RunOptions,
    summary: &mut Summary,
    progress: &P,
) -> Resolved {
    let examined = (batch.candidates.len() + batch.rejected.len()) as u64;
    summary.examined_messages += examined;
    summary.selected += examined;
    for row in batch.rejected {
        failure(
            summary,
            Stage::Selection,
            Failure::Message {
                id: row.id,
                reason: row.reason,
            },
            progress,
        );
    }
    let mut resolved = BTreeMap::new();
    for row in batch.candidates {
        match resolve_path(&options.data_directory, &row) {
            Ok(path) => {
                resolved.insert(row.id, (row, path));
            }
            Err(error) => failure(
                summary,
                Stage::Selection,
                Failure::Message {
                    id: row.id,
                    reason: error.to_string(),
                },
                progress,
            ),
        }
    }
    resolved
}
pub(super) fn emit<P: Fn(Progress)>(stage: Stage, summary: Summary, progress: &P) {
    progress(Progress {
        stage,
        summary,
        failure: None,
    });
}
pub(super) fn failure<P: Fn(Progress)>(
    summary: &mut Summary,
    stage: Stage,
    failure: Failure,
    progress: &P,
) {
    summary.failed += 1;
    progress(Progress {
        stage,
        summary: *summary,
        failure: Some(failure),
    });
}
