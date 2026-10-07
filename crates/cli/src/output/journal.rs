use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
const MAX_BYTES: u64 = 10 * 1024 * 1024;
const ARCHIVES: u8 = 3;
pub(super) struct Journal {
    path: PathBuf,
    file: Option<File>,
    size: u64,
    limit: u64,
}
impl Journal {
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::open_with_limit(path, MAX_BYTES)
    }
    pub fn open_with_limit(path: &Path, limit: u64) -> io::Result<Self> {
        if !path.is_absolute() {
            return Err(io::Error::other("log path must be absolute"));
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let size = file.metadata()?.len();
        Ok(Self {
            path: path.to_path_buf(),
            file: Some(file),
            size,
            limit,
        })
    }
    fn archive(&self, index: u8) -> PathBuf {
        let mut name = self.path.as_os_str().to_os_string();
        name.push(format!(".{index}"));
        PathBuf::from(name)
    }
    fn rotate(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
        }
        match fs::remove_file(self.archive(ARCHIVES)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        for index in (1..ARCHIVES).rev() {
            match fs::rename(self.archive(index), self.archive(index + 1)) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        fs::rename(&self.path, self.archive(1))?;
        self.file = Some(
            OpenOptions::new()
                .create_new(true)
                .append(true)
                .open(&self.path)?,
        );
        self.size = 0;
        Ok(())
    }
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() as u64 > self.limit {
            return Err(io::Error::other("log event exceeds file size limit"));
        }
        if self.size + bytes.len() as u64 > self.limit {
            self.rotate()?;
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("log is unavailable after rotation error"))?;
        file.write_all(bytes)?;
        file.flush()?;
        self.size += bytes.len() as u64;
        Ok(())
    }
}
