use hmail_cleaner_core::{
    cleanup::*,
    maintenance::mail_service::{MailControl, MailState},
};
use hmail_cleaner_runner::{
    lifecycle::{self, PreparedRun},
    MailObservation, Mode, Report, RunObserver, RunResult, Status,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
#[derive(Clone, Copy)]
pub enum Restore {
    Succeeds,
    Fails,
}
#[derive(Clone)]
pub struct Cleanup {
    pub restorations: Arc<AtomicUsize>,
    pub restore: Restore,
}
struct RepositoryFixture;
impl Repository for RepositoryFixture {
    async fn snapshot(&mut self) -> PortResult<Snapshot> {
        Ok(Snapshot { upper_id: 2 })
    }
    async fn next_batch(&mut self, _: Selection, cursor: i64, _: u32) -> PortResult<Batch> {
        if cursor != 0 {
            return Err("original-CLEANUP-CANARY mysql://credential:password@private".into());
        }
        Ok(Batch {
            last_id: 1,
            candidates: vec![],
            rejected: vec![RowFailure {
                id: 1,
                reason: "row-CREDENTIAL-CANARY".into(),
            }],
        })
    }
    async fn delete_batch(&mut self, _: &[Candidate]) -> PortResult<DeleteResult> {
        panic!("rejected rows must never be deleted")
    }
    async fn verify_deleted(&mut self, _: &[i64]) -> PortResult<()> {
        panic!("no committed deletion")
    }
}
struct Files;
impl FileStore for Files {
    async fn inspect(&self, files: Vec<FileTarget>, _: u8) -> PortResult<Vec<FileInspection>> {
        assert!(files.is_empty());
        Ok(vec![])
    }
    async fn delete(&self, _: Vec<FileInspection>, _: u8) -> PortResult<Vec<FileOutcome>> {
        panic!("no valid files")
    }
    async fn prune_empty(&self, _: Vec<PathBuf>) -> PortResult<PruneOutcome> {
        panic!("no valid files")
    }
}
struct Guard;
impl OfflineGuard for Guard {
    async fn ensure_stopped(&self) -> PortResult<()> {
        Ok(())
    }
}
struct Prepared<'a, O> {
    observer: &'a O,
    cancel: &'a Cancellation,
}
impl<O: RunObserver> PreparedRun for Prepared<'_, O> {
    async fn execute(&mut self) -> Report {
        let mut repository = RepositoryFixture;
        let options = RunOptions {
            cutoff: chrono::NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            file_cutoff: std::time::SystemTime::UNIX_EPOCH,
            batch_size: 10.try_into().unwrap(),
            workers: 1,
            data_directory: std::env::temp_dir(),
        };
        let error = hmail_cleaner_core::cleanup::clean(
            Dependencies {
                repository: &mut repository,
                files: &Files,
                guard: &Guard,
            },
            &options,
            self.cancel,
            |progress| self.observer.progress(progress),
        )
        .await
        .unwrap_err();
        Report {
            mode: Some(Mode::Clean),
            status: Status::Failed,
            summary: *error.summary,
            stage: Some(error.stage),
            error: Some(error.source.to_string()),
            exit_code: 1,
        }
    }
    fn output_failed(&self) -> bool {
        self.observer.failed()
    }
    async fn close(self) -> PortResult<()> {
        Ok(())
    }
}
struct Control<'a, O> {
    observer: &'a O,
    cleanup: &'a Cleanup,
}
impl<O: RunObserver> MailControl for Control<'_, O> {
    async fn state(&self) -> PortResult<MailState> {
        self.observer
            .service_state(MailObservation::State(MailState::Running));
        Ok(MailState::Running)
    }
    async fn stop(&self) -> PortResult<()> {
        Ok(())
    }
    async fn wait_for_stopped(&self, _: Duration) -> PortResult<()> {
        self.observer
            .service_state(MailObservation::State(MailState::Stopped));
        Ok(())
    }
    async fn restore_running(&self, _: Duration) -> PortResult<()> {
        self.observer.service_state(MailObservation::Restoring);
        self.cleanup.restorations.fetch_add(1, Ordering::SeqCst);
        if matches!(self.cleanup.restore, Restore::Fails) {
            return Err("RESTORE-CANARY password=secret".into());
        }
        self.observer
            .service_state(MailObservation::State(MailState::Running));
        Ok(())
    }
}
impl hmail_cleaner_agent::execution::Executor for Cleanup {
    async fn execute(
        &self,
        _: &hmail_cleaner_protocol::ValidatedJob,
        observer: &impl RunObserver,
        cancel: &Cancellation,
    ) -> RunResult {
        let result = lifecycle::run(
            Prepared { observer, cancel },
            Some(&Control {
                observer,
                cleanup: self,
            }),
            cancel,
        )
        .await;
        let report = match &result {
            RunResult::Finished(report) | RunResult::Fatal(report) => report,
        };
        assert!(
            report
                .error
                .as_ref()
                .unwrap()
                .contains("original-CLEANUP-CANARY"),
            "journal must never replace the original execution failure"
        );
        if matches!(self.restore, Restore::Fails) {
            assert!(report.error.as_ref().unwrap().contains("RESTORE-CANARY"));
        }
        result
    }
}
