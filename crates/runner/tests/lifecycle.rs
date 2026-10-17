use hmail_cleaner_core::{
    cleanup::{Cancellation, PortResult, Stage, Summary},
    maintenance::mail_service::{MailControl, MailState},
};
use hmail_cleaner_runner::{
    lifecycle::{run, PreparedRun},
    Mode, Report, Status,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

struct Job {
    trace: Arc<Mutex<Vec<&'static str>>>,
    close_fails: bool,
}

impl PreparedRun for Job {
    async fn execute(&mut self) -> Report {
        self.trace.lock().unwrap().push("work");
        Report {
            mode: Some(Mode::Clean),
            status: Status::Failed,
            summary: Summary {
                removed_files: 2,
                ..Summary::default()
            },
            stage: Some(Stage::Files),
            error: Some("fixture cleanup failure".into()),
            exit_code: 1,
        }
    }
    fn output_failed(&self) -> bool {
        false
    }
    async fn close(self) -> PortResult<()> {
        self.trace.lock().unwrap().push("release lock");
        if self.close_fails {
            return Err("fixture close failure".into());
        }
        Ok(())
    }
}

struct Control {
    trace: Arc<Mutex<Vec<&'static str>>>,
    restore_fails: bool,
}

impl MailControl for Control {
    async fn state(&self) -> PortResult<MailState> {
        Ok(MailState::Running)
    }
    async fn stop(&self) -> PortResult<()> {
        self.trace.lock().unwrap().push("stop");
        Ok(())
    }
    async fn wait_for_stopped(&self, _: Duration) -> PortResult<()> {
        self.trace.lock().unwrap().push("stopped");
        Ok(())
    }
    async fn restore_running(&self, _: Duration) -> PortResult<()> {
        let mut trace = self.trace.lock().unwrap();
        assert!(!trace.contains(&"release lock"));
        trace.push("restore");
        if self.restore_fails {
            return Err("fixture restore failure".into());
        }
        Ok(())
    }
}

#[tokio::test]
async fn database_lock_is_held_until_restore_finishes() {
    let trace = Arc::new(Mutex::new(vec!["acquire lock", "verify access"]));
    let result = run(
        Job {
            trace: trace.clone(),
            close_fails: false,
        },
        Some(&Control {
            trace: trace.clone(),
            restore_fails: false,
        }),
        &Cancellation::default(),
    )
    .await;
    assert!(!result.is_fatal());
    assert_eq!(
        *trace.lock().unwrap(),
        [
            "acquire lock",
            "verify access",
            "stop",
            "stopped",
            "work",
            "restore",
            "release lock"
        ]
    );
    assert_eq!(result.into_report().summary.removed_files, 2);
}

#[tokio::test]
async fn restore_failure_preserves_cleanup_failure_and_partial_counters() {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let result = run(
        Job {
            trace: trace.clone(),
            close_fails: false,
        },
        Some(&Control {
            trace: trace.clone(),
            restore_fails: true,
        }),
        &Cancellation::default(),
    )
    .await;
    assert!(result.is_fatal());
    let report = result.into_report();
    let error = report.error.unwrap();
    assert!(error.contains("fixture cleanup failure"));
    assert!(error.contains("fixture restore failure"));
    assert_eq!(report.summary.removed_files, 2);
    assert_eq!(report.exit_code, 1);
    assert_eq!(trace.lock().unwrap().last(), Some(&"release lock"));
}

#[tokio::test]
async fn close_failure_preserves_cleanup_failure_and_stage() {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let result = run(
        Job {
            trace: trace.clone(),
            close_fails: true,
        },
        Some(&Control {
            trace,
            restore_fails: false,
        }),
        &Cancellation::default(),
    )
    .await;
    let report = result.into_report();
    let error = report.error.unwrap();
    assert!(error.contains("fixture cleanup failure"));
    assert!(error.contains("fixture close failure"));
    assert_eq!(report.stage, Some(Stage::Files));
    assert_eq!(report.summary.removed_files, 2);
    assert_eq!(report.exit_code, 1);
}
