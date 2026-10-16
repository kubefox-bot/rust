mod console;
mod dashboard;
mod event;
mod journal;
mod terminal;
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
const BYTES_PER_GIB: f64 = 1024.0 * 1024.0 * 1024.0;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputPolicy {
    Text,
    Plain,
    Json,
    Silent,
}
pub(crate) struct Output {
    policy: OutputPolicy,
    last: Mutex<Option<Instant>>,
    journal: Mutex<Option<journal::Journal>>,
    failed: Arc<AtomicBool>,
    dashboard: Mutex<Option<dashboard::Dashboard>>,
    stage: Mutex<Option<Stage>>,
    preview: AtomicBool,
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
                "--silence требует --log-file или HMAIL_LOG_FILE",
            ));
        }
        Ok(Self {
            policy,
            last: Mutex::new(None),
            journal: Mutex::new(log_file.map(journal::Journal::open).transpose()?),
            failed: Arc::new(AtomicBool::new(false)),
            dashboard: Mutex::new(None),
            stage: Mutex::new(None),
            preview: AtomicBool::new(false),
            cancel,
        })
    }
    pub fn begin(&self, config: &crate::config::Config) {
        use std::io::IsTerminal;
        *self.last.lock().unwrap() = None;
        *self.stage.lock().unwrap() = None;
        let preview = config.mode == crate::args::Mode::Preview;
        self.preview.store(preview, Ordering::Relaxed);
        let source = config
            .env_file
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "переменные окружения и аргументы".into());
        let scan = match config.scan {
            hmail_cleaner_core::maintenance::ScanOptions::Fast => "Быстрая",
            hmail_cleaner_core::maintenance::ScanOptions::Full => "Полная",
        };
        let context = vec![
            format!("Настройки: {source}"),
            format!("Корень: {}", config.options.data_directory.display()),
            format!("Дата создания строго раньше: {}", config.options.cutoff),
        ];
        if self.policy == OutputPolicy::Text
            && std::io::stdin().is_terminal()
            && std::io::stdout().is_terminal()
            && std::io::stderr().is_terminal()
        {
            let model = dashboard::Model {
                title: format!(
                    "hMail Cleaner {} | {scan} | {}",
                    env!("HMAIL_BUILD_VERSION"),
                    if preview {
                        "Предпросмотр"
                    } else {
                        "Очистка"
                    }
                ),
                context: context.clone(),
                preview,
                service: "проверяется".into(),
                stage: Stage::Preflight,
                summary: Default::default(),
                started: Instant::now(),
                changed: Instant::now(),
                history: Default::default(),
            };
            match dashboard::Dashboard::start(model,self.failed.clone(),self.cancel.clone()) {
                Ok(dashboard)=>*self.dashboard.lock().unwrap()=Some(dashboard),
                Err(_)=>self.action("Полноэкранный режим недоступен; обычный вывод. Проверьте QuickEdit в свойствах CMD."),
            }
        }
        self.action(&format!(
            "Запуск {} — {scan}; {}",
            env!("HMAIL_BUILD_VERSION"),
            context.join("; ")
        ));
    }
    pub fn mail_state(&self, state: &str) {
        if let Some(dashboard) = self.dashboard.lock().unwrap().as_ref() {
            dashboard.model.lock().unwrap().service = state.to_string();
        }
        self.action(&format!("Почтовая служба: {state}"));
    }
    pub fn action(&self, message: &str) {
        self.write(Event::Action { message });
    }
    pub fn server(&self, version: &str, cutoff: NaiveDateTime) {
        self.write(Event::Server {
            mysql_version: version,
            cutoff,
        });
    }
    pub fn progress(&self, progress: Progress) {
        let mut last = self.last.lock().expect("output state poisoned");
        if let Some(dashboard) = self.dashboard.lock().unwrap().as_ref() {
            let mut model = dashboard.model.lock().unwrap();
            if model.summary != progress.summary {
                model.changed = Instant::now();
            }
            model.stage = progress.stage;
            model.summary = progress.summary;
        }
        let mut stage = self.stage.lock().unwrap();
        let urgent = progress.failure.is_some() || *stage != Some(progress.stage);
        *stage = Some(progress.stage);
        if !urgent && last.is_some_and(|t| t.elapsed() < PROGRESS_INTERVAL) {
            return;
        }
        *last = Some(Instant::now());
        self.write(Event::Progress(&progress));
    }
    pub fn finish(&self, report: &Report) {
        // Join the renderer and restore the console before printing a durable final report.
        self.dashboard.lock().unwrap().take();
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
                let text =
                    console::redact(&console::text(&event, self.preview.load(Ordering::Relaxed)));
                journal.write(
                    format!(
                        "[{}] {text}\n",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                    )
                    .as_bytes(),
                )?;
            }
            match self.policy {
                OutputPolicy::Text | OutputPolicy::Plain => {
                    if let Some(dashboard) = self.dashboard.lock().unwrap().as_ref() {
                        let mut state = dashboard.model.lock().unwrap();
                        let note = match &event {
                            Event::Progress(p) if p.failure.is_none() => {
                                console::stage(p.stage).to_string()
                            }
                            _ => console::text(&event, self.preview.load(Ordering::Relaxed)),
                        };
                        state.note(note);
                        Ok(())
                    } else {
                        console::write(&event, self.preview.load(Ordering::Relaxed))
                    }
                }
                OutputPolicy::Json if matches!(event, Event::Action { .. }) => Ok(()),
                OutputPolicy::Json => io::stdout().lock().write_all(&bytes),
                OutputPolicy::Silent => Ok(()),
            }
        })();
        if let Err(error) = result {
            self.failed.store(true, Ordering::Release);
            self.cancel.request();
            if self.policy != OutputPolicy::Silent {
                self.dashboard.lock().unwrap().take();
                eprintln!("Ошибка вывода: {error}");
            }
        }
    }
}
#[cfg(test)]
mod tests;
