use super::{
    batch::{self, Resolved},
    *,
};

pub(super) async fn select<F: FileStore, P: Fn(Progress)>(
    mut planned: Resolved,
    files: &F,
    options: &RunOptions,
    summary: &mut Summary,
    progress: &P,
) -> PortResult<(Resolved, Vec<FileInspection>)> {
    if planned.is_empty() {
        return Ok((planned, Vec::new()));
    }
    let targets = planned
        .iter()
        .map(|(id, (_, path))| FileTarget {
            id: *id,
            path: path.clone(),
        })
        .collect();
    let observations = files.inspect(targets, options.workers).await?;
    batch::validate_files(
        &planned,
        observations.iter().map(|file| (file.id, &file.path)),
    )?;
    let mut selected = Vec::new();
    for file in observations {
        let eligible = match &file.result {
            Ok(Some(metadata)) => metadata.created_at < options.file_cutoff,
            Ok(None) => planned[&file.id].0.created_at < options.cutoff,
            Err(error) => {
                batch::failure(
                    summary,
                    Stage::Selection,
                    Failure::Message {
                        id: file.id,
                        reason: error.message.clone(),
                    },
                    progress,
                );
                planned.remove(&file.id);
                continue;
            }
        };
        if eligible {
            selected.push(file);
        } else {
            planned.remove(&file.id);
            summary.selected -= 1;
        }
    }
    Ok((planned, selected))
}
