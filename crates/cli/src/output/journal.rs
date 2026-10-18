use hmail_cleaner_adapters::journal::RotatingFile;
use std::{io, path::Path};
const MAX_BYTES: u64 = 10 * 1024 * 1024;
const TOTAL_FILES: u8 = 4;
pub(super) struct Journal(RotatingFile);
impl Journal {
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::open_with_limit(path, MAX_BYTES)
    }
    pub fn open_with_limit(path: &Path, limit: u64) -> io::Result<Self> {
        Ok(Self(RotatingFile::open(path, limit, TOTAL_FILES)?))
    }
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.0.write(bytes)
    }
}
