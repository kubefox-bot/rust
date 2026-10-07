use crate::commands::Report;
use chrono::NaiveDateTime;
use hmail_cleaner_core::cleanup::{Cancellation, Progress, Stage};
use serde::Serialize;
use std::{
    io::{self, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
const PROGRESS_INTERVAL_MS: u64 = 250;
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event<'a> {
    Server {
        mysql_version: &'a str,
        cutoff: NaiveDateTime,
    },
    Progress(&'a Progress),
    Result(&'a Report),
}
pub(crate) struct Output {
    json: bool,
    last: Mutex<Option<Instant>>,
    failed: AtomicBool,
    cancel: Arc<Cancellation>,
}
impl Output {
    pub fn new(json: bool, cancel: Arc<Cancellation>) -> Self {
        Self {
            json,
            last: Mutex::new(None),
            failed: AtomicBool::new(false),
            cancel,
        }
    }
    pub fn server(&self, version: &str, cutoff: NaiveDateTime) {
        self.write(|| {
            if self.json {
                json_line(&Event::Server {
                    mysql_version: version,
                    cutoff,
                })
            } else {
                writeln!(
                    io::stderr().lock(),
                    "MySQL {version}; cutoff {cutoff} (server-local, strict <)"
                )
            }
        });
    }
    pub fn progress(&self, progress: Progress) {
        let mut last = self.last.lock().expect("output state poisoned");
        let urgent = progress.failure.is_some() || progress.stage == Stage::Complete;
        if !urgent
            && last.is_some_and(|time| time.elapsed() < Duration::from_millis(PROGRESS_INTERVAL_MS))
        {
            return;
        }
        *last = Some(Instant::now());
        self.write(||if self.json {json_line(&Event::Progress(&progress))}else {
            if let Some(failure)=&progress.failure {return writeln!(io::stderr().lock(),"{:?}: {:?}",progress.stage,failure);}
            writeln!(io::stderr().lock(),"{:?}: selected={}, files={}, absent={}, database={}, directories={}, failed={}",progress.stage,progress.summary.selected,progress.summary.removed_files,progress.summary.absent_files,progress.summary.deleted_messages,progress.summary.removed_directories,progress.summary.failed)
        });
    }
    pub fn finish(&self, report: &Report) {
        self.write(||if self.json {json_line(&Event::Result(report))}else {writeln!(io::stdout().lock(),"{:?}: selected={}, files={}, absent={}, database={}, directories={}, failed={}{}",report.status,report.summary.selected,report.summary.removed_files,report.summary.absent_files,report.summary.deleted_messages,report.summary.removed_directories,report.summary.failed,report.error.as_ref().map(|error|format!("; {error}")).unwrap_or_default())});
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
    fn write(&self, operation: impl FnOnce() -> io::Result<()>) {
        if let Err(error) = operation() {
            if !self.failed.swap(true, Ordering::AcqRel) {
                eprintln!("output failed: {error}");
            }
            self.cancel.request();
        }
    }
}
fn json_line(event: &Event<'_>) -> io::Result<()> {
    let mut output = io::stdout().lock();
    serde_json::to_writer(&mut output, event).map_err(io::Error::other)?;
    writeln!(output)
}
