use crate::report::Report;
use hmail_cleaner_core::{
    cleanup::{Cancellation, PortResult},
    maintenance::mail_service::{self, MailControl},
};
use std::future::Future;
pub enum RunResult {
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
pub trait PreparedRun: Send {
    fn execute(&mut self) -> impl Future<Output = Report> + Send;
    fn output_failed(&self) -> bool;
    fn action(&self, _message: &str) {}
    fn mail_state(&self, state: &str) {
        self.action(state);
    }
    fn close(self) -> impl Future<Output = PortResult<()>> + Send;
}
pub async fn run<R: PreparedRun, C: MailControl + Sync>(
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
