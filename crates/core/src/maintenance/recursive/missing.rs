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
    cancel: &Cancellation,
    progress: &P,
    map: HashMap<String, Vec<StoredMessage>>,
    summary: &mut Summary,
) -> Result<(), CleanupError> {
    let mut pending = Vec::new();
    for rows in map.into_values() {
        if rows.len() > 1 {
            fail(
                summary,
                rows[0].path.clone(),
                "Неоднозначные ссылки БД на отсутствующий в обходе файл".into(),
                progress,
            );
            continue;
        }
        for row in rows {
            pending.push(row);
            if pending.len() == options.batch_size.get() as usize {
                inspect(
                    deps, options, operation, cancel, progress, &pending, summary,
                )
                .await?;
                pending.clear();
            }
        }
    }
    if !pending.is_empty() {
        inspect(
            deps, options, operation, cancel, progress, &pending, summary,
        )
        .await?;
    }
    Ok(())
}
async fn inspect<
    R: TreeRepository,
    F: FullFileStore,
    G: OfflineGuard,
    P: Fn(Progress) + Send + Sync,
>(
    deps: &mut Dependencies<'_, R, F, G>,
    options: &RunOptions,
    operation: Operation,
    cancel: &Cancellation,
    progress: &P,
    rows: &[StoredMessage],
    summary: &mut Summary,
) -> Result<(), CleanupError> {
    workflow::cancelled(cancel, *summary)?;
    workflow::emit(Stage::Selection, *summary, progress);
    let mut expected = rows
        .iter()
        .map(|r| (r.candidate.id, r))
        .collect::<HashMap<_, _>>();
    let results = workflow::at(
        deps.files
            .inspect(
                rows.iter()
                    .map(|r| FileTarget {
                        id: r.candidate.id,
                        path: r.path.clone(),
                    })
                    .collect(),
                options.workers,
            )
            .await,
        Stage::Selection,
        *summary,
    )?;
    let mut eligible = Vec::new();
    for file in results {
        let row = workflow::at(
            expected
                .remove(&file.id)
                .ok_or_else(|| "Неожиданный результат проверки отсутствия".into()),
            Stage::Selection,
            *summary,
        )?;
        workflow::at(
            if file.path == row.path {
                Ok(())
            } else {
                Err("Путь проверки отсутствия не совпал".into())
            },
            Stage::Selection,
            *summary,
        )?;
        summary.examined_messages += 1;
        match file.result {
            Ok(None) if row.candidate.created_at < options.cutoff => {
                summary.selected += 1;
                summary.absent_files += 1;
                eligible.push(row.clone());
            }
            Ok(Some(metadata)) if metadata.created_at < options.file_cutoff => fail(
                summary,
                file.path,
                "Старый файл существует, но не был обработан обходом".into(),
                progress,
            ),
            Ok(_) => {}
            Err(error) => fail(summary, file.path, error.message, progress),
        }
    }
    workflow::at(
        if expected.is_empty() {
            Ok(())
        } else {
            Err("Проверка отсутствия вернула неполный результат".into())
        },
        Stage::Selection,
        *summary,
    )?;
    if operation == Operation::Clean && !eligible.is_empty() {
        workflow::at(
            deps.guard.ensure_stopped().await,
            Stage::Preflight,
            *summary,
        )?;
        // Recheck absence immediately before SQL cleanup, using the same verified deletion adapter.
        let observations = eligible
            .iter()
            .map(|r| FileInspection {
                id: r.candidate.id,
                path: r.path.clone(),
                result: Ok(None),
            })
            .collect();
        workflow::emit(Stage::Files, *summary, progress);
        let outcomes = workflow::at(
            deps.files.delete(observations, options.workers).await,
            Stage::Files,
            *summary,
        )?;
        let mut remaining = eligible
            .into_iter()
            .map(|r| (r.candidate.id, r))
            .collect::<HashMap<_, _>>();
        let mut confirmed = Vec::new();
        for outcome in outcomes {
            let row = workflow::at(
                remaining
                    .remove(&outcome.id)
                    .ok_or_else(|| "Неожиданный результат подтверждения отсутствия".into()),
                Stage::Files,
                *summary,
            )?;
            workflow::at(
                if outcome.path == row.path {
                    Ok(())
                } else {
                    Err("Путь подтверждения отсутствия не совпал".into())
                },
                Stage::Files,
                *summary,
            )?;
            match outcome.result {
                DeletionResult::AlreadyAbsent => confirmed.push(row),
                DeletionResult::Failed(error) => fail(summary, row.path, error.message, progress),
                DeletionResult::Removed { .. } => {
                    return workflow::at(
                        Err("Файл появился при проверке отсутствия".into()),
                        Stage::Files,
                        *summary,
                    )
                }
            }
        }
        workflow::at(
            if remaining.is_empty() {
                Ok(())
            } else {
                Err("Неполный результат подтверждения отсутствия".into())
            },
            Stage::Files,
            *summary,
        )?;
        commit::run(deps.repository, &confirmed, progress, summary).await?;
    }
    Ok(())
}
