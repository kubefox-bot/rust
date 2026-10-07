use crate::args::Mode;
use hmail_cleaner_core::cleanup::{
    self, Cancellation, CleanupError, Dependencies, FileStore, OfflineGuard, Progress, Repository,
    RunOptions, Stage, Summary,
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
    pub status: Status,
    pub summary: Summary,
    pub stage: Option<Stage>,
    pub error: Option<String>,
    pub exit_code: u8,
}
impl Report {
    pub fn invalid(error: impl ToString) -> Self {
        Self {
            status: Status::Invalid,
            summary: Summary::default(),
            stage: Some(Stage::Preflight),
            error: Some(error.to_string()),
            exit_code: EXIT_INVALID,
        }
    }
    pub fn backend(error: impl ToString, summary: Summary) -> Self {
        Self {
            status: Status::Failed,
            summary,
            stage: Some(Stage::Database),
            error: Some(error.to_string()),
            exit_code: EXIT_FAILED,
        }
    }
}
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
        Mode::Preview => cleanup::preview(dependencies.repository, options, progress).await,
        Mode::Clean => cleanup::clean(dependencies, options, cancel, progress).await,
    };
    match result {
        Ok(summary) if summary.failed == 0 => Report {
            status: Status::Complete,
            summary,
            stage: None,
            error: None,
            exit_code: EXIT_OK,
        },
        Ok(summary) => Report {
            status: Status::Partial,
            summary,
            stage: None,
            error: None,
            exit_code: EXIT_FAILED,
        },
        Err(error) => failure(error),
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
        | Stage::Complete => (Status::Failed, EXIT_FAILED),
    };
    Report {
        status,
        summary: error.summary,
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
            Ok(DeleteResult::Committed)
        }
        async fn verify_deleted(&mut self, _ids: &[i64]) -> PortResult<()> {
            Ok(())
        }
    }
    struct NoFiles;
    impl FileStore for NoFiles {
        async fn delete(
            &self,
            _files: Vec<FileTarget>,
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
