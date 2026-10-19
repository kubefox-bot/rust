//! Isolated SCM fixture: fake mail/database ports, a file-triggered manual job.
//! Never opens MySQL, scans mail or controls a real hMailServer service.
use hmail_cleaner_adapters::service;
use hmail_cleaner_core::{
    cleanup::{Cancellation, PortResult, Stage, Summary},
    maintenance::mail_service::{MailControl, MailState},
};
use hmail_cleaner_runner::{
    lifecycle::{self, PreparedRun},
    Report, Status,
};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
const WAIT_POLL: Duration = Duration::from_millis(50);
const RESTORE_DELAY: Duration = Duration::from_secs(2);
fn record(directory: &Path, event: &str) -> PortResult<()> {
    writeln!(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("trace.log"))?,
        "{event}"
    )?;
    Ok(())
}
struct Prepared {
    directory: PathBuf,
    cancel: Arc<Cancellation>,
}
impl PreparedRun for Prepared {
    async fn execute(&mut self) -> Report {
        if let Err(error) = record(&self.directory, "runner started") {
            return Report::invalid(error);
        }
        while !self.cancel.is_requested() {
            tokio::time::sleep(WAIT_POLL).await;
        }
        Report {
            mode: None,
            status: Status::Cancelled,
            summary: Summary::default(),
            stage: Some(Stage::Cancelled),
            error: None,
            exit_code: 130,
        }
    }
    fn output_failed(&self) -> bool {
        false
    }
    async fn close(self) -> PortResult<()> {
        record(&self.directory, "database closed")
    }
}
struct Mail(PathBuf);
impl MailControl for Mail {
    async fn state(&self) -> PortResult<MailState> {
        Ok(MailState::Running)
    }
    async fn stop(&self) -> PortResult<()> {
        record(&self.0, "mail stopped")
    }
    async fn wait_for_stopped(&self, _: Duration) -> PortResult<()> {
        Ok(())
    }
    async fn restore_running(&self, _: Duration) -> PortResult<()> {
        record(&self.0, "restoring")?;
        tokio::time::sleep(RESTORE_DELAY).await;
        record(&self.0, "restored")
    }
}
fn main() {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.len() != 3 || !arguments[1].starts_with("hMailCleanerAgentFixture_") {
        std::process::exit(2);
    }
    let directory = PathBuf::from(&arguments[2]);
    if !directory.is_absolute() || !directory.is_dir() {
        std::process::exit(2);
    }
    let result = service::host(
        &arguments[1],
        Box::new(move |stop| {
            record(&directory, "service idle")?;
            while !stop.is_requested() && !directory.join("manual.command").exists() {
                std::thread::sleep(WAIT_POLL);
            }
            if stop.is_requested() {
                return Ok(());
            }
            let prepared = Prepared {
                directory: directory.clone(),
                cancel: stop.clone(),
            };
            let mail = Mail(directory.clone());
            let result = tokio::runtime::Runtime::new()?.block_on(lifecycle::run(
                prepared,
                Some(&mail),
                &stop,
            ));
            if result.is_fatal() {
                return Err("fixture restoration failed".into());
            }
            Ok(())
        }),
    );
    if result.is_err() {
        std::process::exit(1);
    }
}
