use hmail_cleaner_agent::{
    execution::{Executor, Worker},
    jobs::JobRepository,
};
use hmail_cleaner_core::{
    cleanup::{Cancellation, Progress, Stage, Summary},
    maintenance::{recursive::SelectedFile, FileObservation},
};
use hmail_cleaner_protocol::{validate_job, JobCommand, JobPhase, PreviewFilesQuery, ValidatedJob};
use hmail_cleaner_runner::{MailObservation, Report, RunObserver, RunResult, Status};
use std::{
    future::{poll_fn, Future},
    sync::Arc,
    time::{Duration, SystemTime},
};

struct CaptureWithProgress;
impl Executor for CaptureWithProgress {
    async fn execute(
        &self,
        _: &ValidatedJob,
        observer: &impl RunObserver,
        _: &Cancellation,
    ) -> RunResult {
        let files = (0..1000)
            .map(|index| SelectedFile {
                file: FileObservation {
                    path: format!("D:/Mail/{index}.eml").into(),
                    guid: Default::default(),
                    size_bytes: 7,
                    modified_at: SystemTime::UNIX_EPOCH,
                    created_at: SystemTime::UNIX_EPOCH,
                },
                orphan: true,
            })
            .collect::<Vec<_>>();
        let capture = observer.selected_files(&files);
        tokio::pin!(capture);
        let mut reported = false;
        let result = poll_fn(|cx| {
            let result = capture.as_mut().poll(cx);
            if result.is_pending() && !reported {
                reported = true;
                observer.progress(Progress {
                    stage: Stage::Selection,
                    summary: Summary::default(),
                    failure: None,
                });
                observer.service_state(MailObservation::State(
                    hmail_cleaner_core::maintenance::mail_service::MailState::Stopped,
                ));
            }
            result
        })
        .await;
        RunResult::Finished(Report {
            mode: None,
            status: if result.is_ok() {
                Status::Complete
            } else {
                Status::Failed
            },
            summary: Summary {
                selected_files: 1000,
                ..Summary::default()
            },
            stage: None,
            error: result.err().map(|error| error.to_string()),
            exit_code: 0,
        })
    }
}

#[tokio::test]
async fn preview_capture_keeps_running_while_progress_waits_for_the_same_connection() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let command: JobCommand =
        serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap();
    let id = command.job_id;
    let worker = Worker::new(command.agent_id, repo.clone(), CaptureWithProgress);
    let mut events = worker.subscribe();
    worker.submit(validate_job(command).unwrap()).await.unwrap();
    let terminal = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = events.recv().await.unwrap();
            if !snapshot.phase.is_active() {
                break snapshot;
            }
        }
    })
    .await
    .expect("preview and its progress must advance independently");
    assert_eq!(terminal.phase, JobPhase::Completed);
    assert!(worker.is_healthy());
    assert_eq!(
        repo.preview_files(id, &PreviewFilesQuery::default())
            .await
            .unwrap()
            .total
            .get(),
        1000
    );
    worker.shutdown().await;
}
