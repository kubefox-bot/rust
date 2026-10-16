use super::*;
use std::collections::HashMap;

pub(super) async fn run<
    R: TreeRepository,
    F: FullFileStore,
    G: OfflineGuard,
    P: Fn(Progress) + Send + Sync,
>(
    deps: &mut Dependencies<'_, R, F, G>,
    options: &RunOptions,
    operation: Operation,
    progress: &P,
    map: &mut HashMap<String, Vec<StoredMessage>>,
    scanned: ScanBatch,
    summary: &mut Summary,
) -> Result<(), CleanupError> {
    let mut selected = Vec::new();
    let mut linked = HashMap::new();
    for file in scanned.files {
        let rows = map.remove(&path_key(&file.path)).unwrap_or_default();
        summary.examined_messages += rows.len() as u64;
        if file.created_at >= options.file_cutoff {
            continue;
        }
        if rows.len() > 1 {
            fail(
                summary,
                file.path,
                "Несколько записей БД ссылаются на один путь; требуется сверка".into(),
                progress,
            );
            continue;
        }
        summary.selected += rows.len() as u64;
        summary.selected_bytes += file.size_bytes;
        summary.selected_files += 1;
        if rows.is_empty() {
            summary.selected_orphan_files += 1;
        }
        linked.insert(path_key(&file.path), (file.clone(), rows));
        selected.push(file);
    }
    workflow::emit(Stage::Selection, *summary, progress);
    if operation == Operation::Clean && !selected.is_empty() {
        workflow::at(
            deps.guard.ensure_stopped().await,
            Stage::Preflight,
            *summary,
        )?;
        workflow::emit(Stage::Files, *summary, progress);
        let outcomes = workflow::at(
            deps.files.delete_orphans(selected, options.workers).await,
            Stage::Files,
            *summary,
        )?;
        let mut committed = Vec::new();
        for outcome in outcomes {
            let expected = linked.remove(&path_key(&outcome.observation.path));
            let Some((observation, rows)) = expected else {
                return workflow::at(
                    Err("Неожиданный результат удаления файла".into()),
                    Stage::Files,
                    *summary,
                );
            };
            workflow::at(
                if observation == outcome.observation {
                    Ok(())
                } else {
                    Err("Атрибуты результата удаления не совпали".into())
                },
                Stage::Files,
                *summary,
            )?;
            match outcome.result {
                DeletionResult::Removed { size_bytes } => {
                    summary.removed_files += 1;
                    summary.removed_bytes += size_bytes;
                    if rows.is_empty() {
                        summary.removed_orphan_files += 1;
                    }
                }
                DeletionResult::AlreadyAbsent => summary.absent_files += 1,
                DeletionResult::Failed(error) => {
                    fail(summary, observation.path, error.message, progress);
                    continue;
                }
            }
            committed.extend(rows);
        }
        workflow::at(
            if linked.is_empty() {
                Ok(())
            } else {
                Err("Отсутствует результат удаления файла".into())
            },
            Stage::Files,
            *summary,
        )?;
        commit::run(deps.repository, &committed, progress, summary).await?;
    }
    if operation == Operation::Clean && !scanned.completed_directories.is_empty() {
        workflow::emit(Stage::Directories, *summary, progress);
        let pruning = workflow::at(
            deps.files
                .prune_completed(scanned.completed_directories)
                .await,
            Stage::Directories,
            *summary,
        )?;
        summary.removed_directories += pruning.removed;
        for error in pruning.failures {
            summary.failed += 1;
            progress(Progress {
                stage: Stage::Directories,
                summary: *summary,
                failure: Some(Failure::Directory {
                    path: error.path,
                    reason: error.reason,
                }),
            });
        }
    }
    workflow::emit(Stage::Scan, *summary, progress);
    Ok(())
}
