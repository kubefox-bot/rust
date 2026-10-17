use russh_sftp::protocol::{FileAttributes, Handle, OpenFlags, Status, StatusCode, Version};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
pub struct Sink {
    pub bytes: AtomicUsize,
    pub max_write: AtomicUsize,
    pub fail_after: AtomicUsize,
    hash: Mutex<ring::digest::Context>,
}
impl Default for Sink {
    fn default() -> Self {
        Self {
            bytes: AtomicUsize::new(0),
            max_write: AtomicUsize::new(0),
            fail_after: AtomicUsize::new(usize::MAX),
            hash: Mutex::new(ring::digest::Context::new(&ring::digest::SHA256)),
        }
    }
}
impl Sink {
    pub fn digest(&self) -> String {
        self.hash
            .lock()
            .unwrap()
            .clone()
            .finish()
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}
pub struct Receiver(pub Arc<Sink>);
fn status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: String::new(),
    }
}
impl russh_sftp::server::Handler for Receiver {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }
    async fn init(
        &mut self,
        _: u32,
        _: std::collections::HashMap<String, String>,
    ) -> Result<Version, StatusCode> {
        Ok(Version::new())
    }
    async fn open(
        &mut self,
        id: u32,
        path: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> Result<Handle, StatusCode> {
        if !path.contains("/hMailCleanerAgentStage/")
            || flags.bits() != (OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE).bits()
        {
            return Err(StatusCode::BadMessage);
        }
        Ok(Handle {
            id,
            handle: "fixture".into(),
        })
    }
    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, StatusCode> {
        if handle != "fixture" || offset != self.0.bytes.load(Ordering::SeqCst) as u64 {
            return Err(StatusCode::BadMessage);
        }
        if self.0.bytes.load(Ordering::SeqCst) >= self.0.fail_after.load(Ordering::SeqCst) {
            return Err(StatusCode::Failure);
        }
        self.0.max_write.fetch_max(data.len(), Ordering::SeqCst);
        self.0.hash.lock().unwrap().update(&data);
        self.0.bytes.fetch_add(data.len(), Ordering::SeqCst);
        Ok(status(id))
    }
    async fn close(&mut self, id: u32, _: String) -> Result<Status, StatusCode> {
        Ok(status(id))
    }
}
