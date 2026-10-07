use super::validation;
use hmail_cleaner_core::cleanup::{DeletionResult, FileOutcome, FileTarget, PortResult};
use std::sync::Arc;
pub(super) async fn run<F>(
    groups: Vec<Vec<FileTarget>>,
    workers: usize,
    operation: F,
) -> PortResult<Vec<FileOutcome>>
where
    F: Fn(Vec<FileTarget>) -> Vec<FileOutcome> + Send + Sync + 'static,
{
    if groups.is_empty() {
        return Ok(Vec::new());
    }
    let count = workers.min(groups.len());
    let mut partitions = (0..count).map(|_| Vec::new()).collect::<Vec<_>>();
    for (index, group) in groups.into_iter().enumerate() {
        partitions[index % count].push(group);
    }
    let operation = Arc::new(operation);
    let mut handles = Vec::with_capacity(count);
    for partition in partitions {
        let operation = operation.clone();
        let recovery = partition.iter().flatten().cloned().collect::<Vec<_>>();
        handles.push((
            recovery,
            tokio::task::spawn_blocking(move || {
                partition
                    .into_iter()
                    .flat_map(|group| operation(group))
                    .collect::<Vec<_>>()
            }),
        ));
    }
    let mut outcomes = Vec::new();
    for (recovery, handle) in handles {
        match handle.await {
            Ok(result) => outcomes.extend(result),
            Err(error) => outcomes.extend(recovery.into_iter().map(|file| FileOutcome {
                id: file.id,
                path: file.path,
                result: DeletionResult::Failed(validation::invalid(&format!(
                    "file worker failed: {error}"
                ))),
            })),
        }
    }
    Ok(outcomes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use hmail_cleaner_core::cleanup::DeletionResult;
    use std::{
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };
    #[tokio::test]
    async fn workers_never_exceed_configured_limit() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let observed = peak.clone();
        let running = active.clone();
        let groups = (0..30)
            .map(|id| {
                vec![FileTarget {
                    id,
                    path: PathBuf::from(format!("{id}/mail.eml")),
                }]
            })
            .collect();
        let outcomes = run(groups, 3, move |group| {
            let count = running.fetch_add(1, Ordering::SeqCst) + 1;
            observed.fetch_max(count, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(2));
            running.fetch_sub(1, Ordering::SeqCst);
            group
                .into_iter()
                .map(|file| FileOutcome {
                    id: file.id,
                    path: file.path,
                    result: DeletionResult::Removed { size_bytes: 7 },
                })
                .collect()
        })
        .await
        .unwrap();
        assert_eq!(outcomes.len(), 30);
        assert!(peak.load(Ordering::SeqCst) <= 3);
        assert!(peak.load(Ordering::SeqCst) > 1);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn failed_worker_keeps_every_target_and_joins_other_workers() {
        let groups = (0..2)
            .map(|id| {
                vec![FileTarget {
                    id,
                    path: PathBuf::from(format!("{id}/mail.eml")),
                }]
            })
            .collect();
        let outcomes = run(groups, 2, |group| {
            if group[0].id == 0 {
                panic!("fixture worker panic");
            }
            group
                .into_iter()
                .map(|file| FileOutcome {
                    id: file.id,
                    path: file.path,
                    result: DeletionResult::Removed { size_bytes: 7 },
                })
                .collect()
        })
        .await
        .unwrap();
        assert_eq!(outcomes.len(), 2);
        assert!(matches!(
            outcomes.iter().find(|file| file.id == 0).unwrap().result,
            DeletionResult::Failed(_)
        ));
        assert_eq!(
            outcomes.iter().find(|file| file.id == 1).unwrap().result,
            DeletionResult::Removed { size_bytes: 7 }
        );
    }
}
