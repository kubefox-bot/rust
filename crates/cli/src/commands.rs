use crate::args::Mode;
#[cfg(test)]
use hmail_cleaner_core::cleanup::{self, FileStore, Repository};
use hmail_cleaner_core::cleanup::{
    Cancellation, CleanupError, Dependencies, OfflineGuard, Progress, RunOptions, Stage, Summary,
};
use serde::Serialize;
const EXIT_OK: u8 = 0;
const EXIT_FAILED: u8 = 1;
const EXIT_INVALID: u8 = 2;
const EXIT_CANCELLED: u8 = 130;
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Status {
    Complete,
    Partial,
    Failed,
    Invalid,
    Cancelled,
}
#[derive(Debug, Serialize)]
pub(crate) struct Report {
    pub mode: Option<Mode>,
    pub status: Status,
    pub summary: Summary,
    pub stage: Option<Stage>,
    pub error: Option<String>,
    pub exit_code: u8,
}
impl Report {
    pub fn with_restore_failure(self, error: impl ToString) -> Self {
        let reason = format!(
            "Не удалось восстановить почтовую службу: {error}",
            error = error.to_string()
        );
        Self {
            status: Status::Failed,
            exit_code: EXIT_FAILED,
            error: Some(match self.error {
                Some(original) => format!("{original}; {reason}"),
                None => reason,
            }),
            ..self
        }
    }
    pub fn invalid(error: impl ToString) -> Self {
        Self {
            mode: None,
            status: Status::Invalid,
            summary: Summary::default(),
            stage: Some(Stage::Preflight),
            error: Some(error.to_string()),
            exit_code: EXIT_INVALID,
        }
    }
    pub fn backend(error: impl ToString, summary: Summary) -> Self {
        Self {
            mode: None,
            status: Status::Failed,
            summary,
            stage: Some(Stage::Database),
            error: Some(error.to_string()),
            exit_code: EXIT_FAILED,
        }
    }
    pub fn with_close_failure(self, error: impl ToString) -> Self {
        if self.exit_code == EXIT_OK {
            return Self::backend(error, self.summary);
        }
        let close_error = error.to_string();
        let message = match &self.error {
            Some(original) => {
                format!("{original}; Закрытие БД также завершилось ошибкой: {close_error}")
            }
            None => format!("Закрытие БД также завершилось ошибкой: {close_error}"),
        };
        Self {
            error: Some(message),
            ..self
        }
    }
}
#[cfg(test)]
pub(crate) async fn execute<
    R: Repository,
    F: FileStore,
    G: OfflineGuard,
    P: Fn(Progress) + Send + Sync,
>(
    mode: Mode,
    dependencies: Dependencies<'_, R, F, G>,
    options: &RunOptions,
    cancel: &Cancellation,
    progress: P,
) -> Report {
    let result = match mode {
        Mode::Preview => {
            cleanup::preview(
                dependencies.repository,
                dependencies.files,
                options,
                progress,
            )
            .await
        }
        Mode::Clean => cleanup::clean(dependencies, options, cancel, progress).await,
    };
    result_report(mode, result)
}
pub(crate) async fn execute_scan<
    R: hmail_cleaner_core::maintenance::recursive::TreeRepository,
    F: hmail_cleaner_core::maintenance::FullFileStore,
    G: OfflineGuard,
    P: Fn(Progress) + Send + Sync,
>(
    mode: Mode,
    dependencies: Dependencies<'_, R, F, G>,
    options: &RunOptions,
    scan: hmail_cleaner_core::maintenance::ScanOptions,
    cancel: &Cancellation,
    progress: P,
) -> Report {
    result_report(
        mode,
        hmail_cleaner_core::maintenance::recursive::run(
            dependencies,
            options,
            scan,
            cancel,
            progress,
            mode.action(),
        )
        .await,
    )
}

fn result_report(mode: Mode, result: Result<Summary, CleanupError>) -> Report {
    match result {
        Ok(summary) if summary.failed == 0 => Report {
            mode: Some(mode),
            status: Status::Complete,
            summary,
            stage: None,
            error: None,
            exit_code: EXIT_OK,
        },
        Ok(summary) => Report {
            mode: Some(mode),
            status: Status::Partial,
            summary,
            stage: None,
            error: None,
            exit_code: EXIT_FAILED,
        },
        Err(error) => Report {
            mode: Some(mode),
            ..failure(error)
        },
    }
}
fn failure(error: CleanupError) -> Report {
    let (status, exit_code) = match error.stage {
        Stage::Cancelled => (Status::Cancelled, EXIT_CANCELLED),
        Stage::Preflight => (Status::Invalid, EXIT_INVALID),
        Stage::Snapshot
        | Stage::Selection
        | Stage::Files
        | Stage::Database
        | Stage::Verification
        | Stage::Directories
        | Stage::OrphanDatabase
        | Stage::References
        | Stage::Scan
        | Stage::Complete => (Status::Failed, EXIT_FAILED),
    };
    Report {
        mode: None,
        status,
        summary: *error.summary,
        stage: Some(error.stage),
        error: Some(error.source.to_string()),
        exit_code,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Mode;
    use chrono::NaiveDateTime;
    use hmail_cleaner_core::cleanup::*;
    use std::{num::NonZeroU32, path::PathBuf};
    struct Repo {
        deletes: usize,
    }
    impl Repository for Repo {
        async fn snapshot(&mut self) -> PortResult<Snapshot> {
            Ok(Snapshot { upper_id: 1 })
        }
        async fn next_batch(
            &mut self,
            _selection: Selection,
            cursor: i64,
            _limit: u32,
        ) -> PortResult<Batch> {
            Ok(if cursor == 0 {
                Batch {
                    last_id: 1,
                    candidates: Vec::new(),
                    rejected: vec![RowFailure {
                        id: 1,
                        reason: "invalid row".into(),
                    }],
                }
            } else {
                Batch {
                    last_id: cursor,
                    candidates: Vec::new(),
                    rejected: Vec::new(),
                }
            })
        }
        async fn delete_batch(&mut self, _rows: &[Candidate]) -> PortResult<DeleteResult> {
            self.deletes += 1;
            Ok(DeleteResult::Committed(
                hmail_cleaner_core::cleanup::DeletedRows::default(),
            ))
        }
        async fn verify_deleted(&mut self, _ids: &[i64]) -> PortResult<()> {
            Ok(())
        }
    }
    struct NoFiles;
    impl FileStore for NoFiles {
        async fn inspect(&self, _: Vec<FileTarget>, _: u8) -> PortResult<Vec<FileInspection>> {
            panic!("no file calls expected")
        }
        async fn delete(
            &self,
            _files: Vec<FileInspection>,
            _workers: u8,
        ) -> PortResult<Vec<FileOutcome>> {
            panic!("no file calls expected")
        }
        async fn prune_empty(&self, _parents: Vec<PathBuf>) -> PortResult<PruneOutcome> {
            panic!("no prune calls expected")
        }
    }
    struct Guard;
    impl OfflineGuard for Guard {
        async fn ensure_stopped(&self) -> PortResult<()> {
            Ok(())
        }
    }
    fn options() -> RunOptions {
        RunOptions {
            cutoff: NaiveDateTime::parse_from_str("2026-09-07 12:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            file_cutoff: std::time::SystemTime::now(),
            batch_size: NonZeroU32::new(1000).unwrap(),
            workers: 2,
            data_directory: PathBuf::from("/Data"),
        }
    }
    #[tokio::test]
    async fn preview_is_read_only() {
        let mut repo = Repo { deletes: 0 };
        let report = execute(
            Mode::Preview,
            Dependencies {
                repository: &mut repo,
                files: &NoFiles,
                guard: &Guard,
            },
            &options(),
            &Cancellation::default(),
            |_| {},
        )
        .await;
        assert_eq!(report.summary.selected, 1);
        assert_eq!(repo.deletes, 0);
    }
    #[tokio::test]
    async fn partial_cleanup_has_nonzero_exit() {
        let mut repo = Repo { deletes: 0 };
        let report = execute(
            Mode::Clean,
            Dependencies {
                repository: &mut repo,
                files: &NoFiles,
                guard: &Guard,
            },
            &options(),
            &Cancellation::default(),
            |_| {},
        )
        .await;
        assert_eq!(report.exit_code, 1);
        assert_eq!(report.summary.failed, 1);
    }
}

#[cfg(test)]
mod close_tests {
    use super::*;
    #[test]
    fn close_failure_preserves_uncertain_commit() {
        let original = Report {
            mode: None,
            status: Status::Failed,
            summary: Summary {
                removed_files: 2,
                ..Summary::default()
            },
            stage: Some(Stage::Database),
            error: Some("COMMIT confirmation lost; repeat cleanup to reconcile".into()),
            exit_code: 1,
        };
        let report = original.with_close_failure("broken pipe during QUIT");
        assert!(report
            .error
            .as_deref()
            .unwrap()
            .contains("COMMIT confirmation lost"));
        assert!(report.error.as_deref().unwrap().contains("broken pipe"));
        assert_eq!(report.summary.removed_files, 2);
        assert_eq!(report.stage, Some(Stage::Database));
        assert_eq!(report.exit_code, 1);
    }
    #[test]
    fn close_failure_preserves_cancelled_and_preflight_reports() {
        for (status, stage, exit_code) in [
            (Status::Cancelled, Stage::Cancelled, 130),
            (Status::Invalid, Stage::Preflight, 2),
        ] {
            let original = Report {
                mode: None,
                status,
                summary: Summary::default(),
                stage: Some(stage),
                error: Some("original failure".into()),
                exit_code,
            };
            let report = original.with_close_failure("QUIT failure");
            assert_eq!(report.exit_code, exit_code);
            assert_eq!(report.stage, Some(stage));
            assert!(report
                .error
                .as_deref()
                .unwrap()
                .contains("original failure"));
        }
    }
    #[test]
    fn close_failure_turns_success_into_failure() {
        let original = Report {
            mode: None,
            status: Status::Complete,
            summary: Summary {
                deleted_messages: 2,
                ..Summary::default()
            },
            stage: None,
            error: None,
            exit_code: 0,
        };
        let report = original.with_close_failure("QUIT failure");
        assert_eq!(report.exit_code, 1);
        assert_eq!(report.summary.deleted_messages, 2);
        assert!(report.error.as_deref().unwrap().contains("QUIT failure"));
    }
}
