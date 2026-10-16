mod model;
use super::terminal;
use crossterm::{cursor, terminal as screen, QueueableCommand};
use hmail_cleaner_core::cleanup::Cancellation;
pub(super) use model::Model;
use std::{
    io::{self, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
const TICK: Duration = Duration::from_millis(100);
const FALLBACK_SIZE: (u16, u16) = (80, 25);
pub(super) struct Dashboard {
    stop: mpsc::Sender<()>,
    worker: Option<JoinHandle<()>>,
    pub model: Arc<Mutex<Model>>,
}
impl Dashboard {
    pub fn start(
        model: Model,
        failed: Arc<AtomicBool>,
        cancel: Arc<Cancellation>,
    ) -> io::Result<Self> {
        let model = Arc::new(Mutex::new(model));
        let state = model.clone();
        let (stop, receiver) = mpsc::channel();
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let _session = match terminal::Session::open() {
                Ok(session) => {
                    let _ = ready.send(Ok(()));
                    session
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            };
            let mut tick = 0;
            let mut previous = Vec::new();
            let mut dimensions = (0, 0);
            loop {
                let (width, height) = screen::size().unwrap_or(FALLBACK_SIZE);
                let resized = dimensions != (width, height);
                let lines = state.lock().unwrap_or_else(|e| e.into_inner()).lines(
                    usize::from(width.saturating_sub(1)),
                    usize::from(height),
                    tick,
                );
                let result = (|| {
                    let mut writer = io::stderr().lock();
                    if resized {
                        writer.queue(screen::Clear(screen::ClearType::All))?;
                    }
                    for (row, line) in lines.iter().enumerate() {
                        if !resized && previous.get(row) == Some(line) {
                            continue;
                        }
                        writer
                            .queue(cursor::MoveTo(0, row as u16))?
                            .queue(screen::Clear(screen::ClearType::CurrentLine))?;
                        write!(writer, "{line}")?;
                    }
                    writer.flush()
                })();
                if result.is_err() {
                    failed.store(true, Ordering::Release);
                    cancel.request();
                    break;
                }
                previous = lines;
                dimensions = (width, height);
                tick += 1;
                match receiver.recv_timeout(TICK) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        });
        match initialized.recv() {
            Ok(Ok(())) => Ok(Self {
                stop,
                worker: Some(worker),
                model,
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(error) => {
                let _ = worker.join();
                Err(io::Error::other(error))
            }
        }
    }
}
impl Drop for Dashboard {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
