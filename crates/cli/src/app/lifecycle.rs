use crate::commands::Report;
use hmail_cleaner_core::{
    cleanup::{Cancellation, PortResult},
    maintenance::mail_service::{self, MailControl},
};
use std::future::Future;
pub(crate) enum RunResult {
    Finished(Report),
    Fatal(Report),
}
impl RunResult {
    pub fn into_report(self) -> Report {
        match self {
            Self::Finished(report) | Self::Fatal(report) => report,
        }
    }
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Fatal(_))
    }
}
pub(super) trait PreparedRun {
    fn execute(&mut self) -> impl Future<Output = Report>;
    fn output_failed(&self) -> bool;
    fn action(&self, _message: &str) {}
    fn mail_state(&self, state: &str) {
        self.action(state);
    }
    fn close(self) -> impl Future<Output = PortResult<()>>;
}
pub(super) async fn run<R: PreparedRun, C: MailControl>(
    mut prepared: R,
    control: Option<&C>,
    cancel: &Cancellation,
) -> RunResult {
    let (report, restore_error) = match control {
        Some(control) => {
            prepared.action(
                "Остановка hMailServer; после обработки исходное состояние будет восстановлено",
            );
            let result = mail_service::with_stopped(control, cancel, || async {
                Ok(prepared.execute().await)
            })
            .await;
            (
                result.result.unwrap_or_else(Report::invalid),
                result.restore_error,
            )
        }
        None => (prepared.execute().await, None),
    };
    if control.is_some() && restore_error.is_none() {
        prepared.mail_state("исходное состояние восстановлено");
    }
    let fatal = restore_error.is_some() || prepared.output_failed();
    let report = match restore_error {
        Some(error) => report.with_restore_failure(error),
        None => report,
    };
    let report = match prepared.close().await {
        Ok(()) => report,
        Err(error) => report.with_close_failure(error),
    };
    if fatal {
        RunResult::Fatal(report)
    } else {
        RunResult::Finished(report)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use hmail_cleaner_core::maintenance::mail_service::MailState;
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };
    struct Job {
        trace: Arc<Mutex<Vec<&'static str>>>,
    }
    impl PreparedRun for Job {
        async fn execute(&mut self) -> Report {
            self.trace.lock().unwrap().push("work");
            Report::invalid("fixture cleanup failure")
        }
        fn output_failed(&self) -> bool {
            false
        }
        async fn close(self) -> PortResult<()> {
            self.trace.lock().unwrap().push("release lock");
            Ok(())
        }
    }
    struct Control {
        trace: Arc<Mutex<Vec<&'static str>>>,
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
            Ok(())
        }
    }
    #[tokio::test]
    async fn database_lock_is_held_until_restore_finishes() {
        let trace = Arc::new(Mutex::new(vec!["acquire lock", "verify access"]));
        let control = Control {
            trace: trace.clone(),
        };
        let result = run(
            Job {
                trace: trace.clone(),
            },
            Some(&control),
            &Cancellation::default(),
        )
        .await;
        assert!(!result.is_fatal());
        assert_eq!(
            *trace.lock().unwrap(),
            vec![
                "acquire lock",
                "verify access",
                "stop",
                "stopped",
                "work",
                "restore",
                "release lock"
            ]
        );
    }
}
