//! Bounded disk transport. Consumers own closed diagnostic schemas and process exclusion.
mod rotation;
pub use rotation::RotatingFile;
use serde::Serialize;
use std::{
    io::{self, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

pub const RUNTIME_FILE_BYTES: u64 = 10 * 1024 * 1024;
pub const RUNTIME_TOTAL_FILES: u8 = 5;
pub const MAX_RECORD_BYTES: usize = 2 * 1024;
#[derive(Clone)]
pub struct RuntimeJournal(Arc<State>);
struct State {
    writer: Mutex<RotatingFile>,
    failed: AtomicBool,
}
struct RecordBuffer {
    bytes: [u8; MAX_RECORD_BYTES],
    used: usize,
}
impl Write for RecordBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.bytes.len() - self.used {
            return Err(io::Error::other("journal record exceeds message limit"));
        }
        self.bytes[self.used..self.used + bytes.len()].copy_from_slice(bytes);
        self.used += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[derive(Serialize)]
struct Record<'a, T> {
    timestamp: chrono::DateTime<chrono::Utc>,
    event: &'a T,
}
impl RuntimeJournal {
    /// Must be opened only by the service after acquiring its protected-directory lease.
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::with_policy(path, RUNTIME_FILE_BYTES, RUNTIME_TOTAL_FILES)
    }
    fn with_policy(path: &Path, limit: u64, total_files: u8) -> io::Result<Self> {
        Ok(Self(Arc::new(State {
            writer: Mutex::new(RotatingFile::open(path, limit, total_files)?),
            failed: AtomicBool::new(false),
        })))
    }
    /// Only closed, consumer-owned safe diagnostic records belong here; never Error/Debug,
    /// arbitrary text, request objects, credentials or profiles. Oversize records are rejected
    /// before disk I/O. Encoding uses no queue or truncation; an I/O failure may leave
    /// an incomplete last record and always latches the writer unavailable.
    pub fn record(&self, event: &impl Serialize) -> io::Result<()> {
        let result = self.write_record(event);
        if result.is_err() && !self.0.failed.swap(true, Ordering::AcqRel) {
            eprintln!("disk journal write failed; diagnostics unavailable");
        }
        result
    }
    fn write_record(&self, event: &impl Serialize) -> io::Result<()> {
        if !self.is_healthy() {
            return Err(io::Error::other("disk journal previously failed"));
        }
        let mut buffer = RecordBuffer {
            bytes: [0; MAX_RECORD_BYTES],
            used: 0,
        };
        serde_json::to_writer(
            &mut buffer,
            &Record {
                timestamp: chrono::Utc::now(),
                event,
            },
        )
        .map_err(|_| io::Error::other("journal record encoding failed"))?;
        buffer.write_all(b"\n")?;
        self.0
            .writer
            .lock()
            .map_err(|_| io::Error::other("journal writer poisoned"))?
            .write(&buffer.bytes[..buffer.used])
    }
    pub fn is_healthy(&self) -> bool {
        !self.0.failed.load(Ordering::Acquire)
    }
}
#[cfg(test)]
mod tests;
