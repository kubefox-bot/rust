use super::layout;
use crate::filesystem::validation;
use hmail_cleaner_core::{
    cleanup::{PortResult, MAX_BATCH_SIZE},
    maintenance::{FileScan, ScanBatch, ScanStats},
};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::task::JoinHandle;

struct Frame {
    entries: fs::ReadDir,
    path: PathBuf,
}
struct Walker {
    stack: Vec<Frame>,
}
impl Walker {
    fn new(root: PathBuf) -> PortResult<Self> {
        validation::directory(&root).map_err(|error| error.message)?;
        Ok(Self {
            stack: vec![Frame {
                entries: fs::read_dir(&root)?,
                path: root,
            }],
        })
    }
    fn batch(&mut self, limit: u32) -> PortResult<ScanBatch> {
        let mut batch = ScanBatch {
            completed_directories: Vec::new(),
            visited_entries: 0,
            examined_files: 0,
            files: Vec::new(),
            finished: false,
        };
        while batch.visited_entries < u64::from(limit) {
            let Some(frame) = self.stack.last_mut() else {
                batch.finished = true;
                break;
            };
            let Some(entry) = frame.entries.next() else {
                if let Some(frame) = self.stack.pop() {
                    batch.completed_directories.push(frame.path);
                }
                continue;
            };
            let entry = entry.map_err(|error| {
                format!(
                    "Не удалось прочитать каталог {}: {error}",
                    frame.path.display()
                )
            })?;
            batch.visited_entries += 1;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                format!("Не удалось прочитать атрибуты {}: {error}", path.display())
            })?;
            if validation::linked(&metadata) {
                continue;
            }
            if metadata.is_file() {
                batch.examined_files += 1;
                if let Some(file) = layout::observation(&path, &metadata).map_err(|error| {
                    format!("Не удалось прочитать даты {}: {error}", path.display())
                })? {
                    batch.files.push(file);
                }
            } else if metadata.is_dir() {
                self.stack.push(Frame {
                    entries: fs::read_dir(&path).map_err(|error| {
                        format!("Не удалось открыть каталог {}: {error}", path.display())
                    })?,
                    path,
                });
            }
        }
        Ok(batch)
    }
}

pub struct Session {
    state: Arc<Mutex<Walker>>,
    pending: Option<JoinHandle<PortResult<ScanBatch>>>,
}
pub(in crate::filesystem) async fn open(root: PathBuf) -> PortResult<Session> {
    let walker = tokio::task::spawn_blocking(move || Walker::new(root)).await??;
    Ok(Session {
        state: Arc::new(Mutex::new(walker)),
        pending: None,
    })
}
pub(in crate::filesystem) async fn check(root: PathBuf) -> PortResult<ScanStats> {
    tokio::task::spawn_blocking(move || {
        let mut walker = Walker::new(root)?;
        let mut examined_files = 0;
        loop {
            let batch = walker.batch(MAX_BATCH_SIZE)?;
            examined_files += batch.examined_files;
            if batch.finished {
                return Ok(ScanStats { examined_files });
            }
        }
    })
    .await?
}
impl FileScan for Session {
    async fn next_batch(&mut self, limit: u32) -> PortResult<ScanBatch> {
        if limit == 0 || limit > MAX_BATCH_SIZE {
            return Err("invalid scan batch size".into());
        }
        if self.pending.is_none() {
            let state = self.state.clone();
            self.pending = Some(tokio::task::spawn_blocking(move || {
                state
                    .lock()
                    .map_err(|_| "scan worker poisoned")?
                    .batch(limit)
            }));
        }
        let outcome = self.pending.as_mut().expect("owned scan worker").await;
        self.pending.take();
        outcome?
    }
    async fn close(mut self) -> PortResult<()> {
        let pending = match self.pending.take() {
            Some(handle) => Some(handle.await),
            None => None,
        };
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .stack
            .clear();
        match pending {
            Some(result) => {
                result??;
                Ok(())
            }
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    #[tokio::test]
    async fn cancelled_pending_batch_is_joined_by_close() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("data");
        fs::create_dir(&root).unwrap();
        let mut session = open(root.clone()).await.unwrap();
        let state = session.state.clone();
        let guard = state.lock().unwrap();
        let mut future = Box::pin(session.next_batch(2));
        assert!(matches!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        drop(future);
        drop(guard);
        session.close().await.unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert!(!root.exists());
    }
}
