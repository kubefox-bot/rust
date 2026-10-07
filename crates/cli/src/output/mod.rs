mod console;
mod event;
mod journal;
use crate::commands::Report;
use chrono::NaiveDateTime;
use event::Event;
use hmail_cleaner_core::cleanup::{Cancellation, Progress, Stage};
use std::{
    io::{self, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputPolicy {
    Text,
    Json,
    Silent,
}
pub(crate) struct Output {
    policy: OutputPolicy,
    last: Mutex<Option<Instant>>,
    journal: Mutex<Option<journal::Journal>>,
    failed: AtomicBool,
    cancel: Arc<Cancellation>,
}
impl Output {
    pub fn open(
        policy: OutputPolicy,
        log_file: Option<&Path>,
        cancel: Arc<Cancellation>,
    ) -> io::Result<Self> {
        if policy == OutputPolicy::Silent && log_file.is_none() {
            return Err(io::Error::other(
                "--silence requires --log-file or HMAIL_LOG_FILE",
            ));
        }
        Ok(Self {
            policy,
            last: Mutex::new(None),
            journal: Mutex::new(log_file.map(journal::Journal::open).transpose()?),
            failed: AtomicBool::new(false),
            cancel,
        })
    }
    pub fn server(&self, version: &str, cutoff: NaiveDateTime) {
        self.write(Event::Server {
            mysql_version: version,
            cutoff,
        });
    }
    pub fn progress(&self, progress: Progress) {
        let mut last = self.last.lock().expect("output state poisoned");
        let urgent = progress.failure.is_some() || progress.stage == Stage::Complete;
        if !urgent && last.is_some_and(|t| t.elapsed() < PROGRESS_INTERVAL) {
            return;
        }
        *last = Some(Instant::now());
        self.write(Event::Progress(&progress));
    }
    pub fn finish(&self, report: &Report) {
        self.write(Event::Result(report));
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
    fn write(&self, event: Event<'_>) {
        if self.failed() {
            return;
        }
        let result = (|| {
            let bytes = event.line()?;
            if let Some(journal) = self
                .journal
                .lock()
                .expect("journal state poisoned")
                .as_mut()
            {
                journal.write(&bytes)?;
            }
            match self.policy {
                OutputPolicy::Text => console::write(&event),
                OutputPolicy::Json => io::stdout().lock().write_all(&bytes),
                OutputPolicy::Silent => Ok(()),
            }
        })();
        if let Err(error) = result {
            self.failed.store(true, Ordering::Release);
            self.cancel.request();
            if self.policy != OutputPolicy::Silent {
                eprintln!("output failed: {error}");
            }
        }
    }
}
#[cfg(test)]
mod tests;
