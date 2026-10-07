use super::layout::{self, Level};
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
    level: Level,
}
struct Walker {
    stack: Vec<Frame>,
}
impl Walker {
    fn new(root: PathBuf) -> PortResult<Self> {
        validation::directory(&root).map_err(|error| error.message)?;
        Ok(Self {
            stack: vec![Frame {
                entries: fs::read_dir(root)?,
                level: Level::Root,
            }],
        })
    }
    fn batch(&mut self, limit: u32) -> PortResult<ScanBatch> {
        let mut batch = ScanBatch {
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
            let level = frame.level;
            let Some(entry) = frame.entries.next() else {
                self.stack.pop();
                continue;
            };
            let entry = entry?;
            batch.visited_entries += 1;
            let name = entry.file_name();
            if matches!(level, Level::Root)
                && name
                    .to_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case("#Public"))
            {
                continue;
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if matches!(level, Level::Bucket) {
                if metadata.is_file() && !validation::linked(&metadata) {
                    batch.examined_files += 1;
                    if let Some(file) = layout::observation(&path, &metadata)? {
                        batch.files.push(file);
                    }
                }
            } else if let Some(level) = name
                .to_str()
                .and_then(|name| layout::child_directory(level, name, &metadata))
            {
                self.stack.push(Frame {
                    entries: fs::read_dir(path)?,
                    level,
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
