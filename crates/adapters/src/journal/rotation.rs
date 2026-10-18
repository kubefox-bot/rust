use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Portable byte rotation. The caller owns formatting, confidentiality and writer exclusion.
pub struct RotatingFile {
    path: PathBuf,
    file: Option<File>,
    size: u64,
    limit: u64,
    archives: u8,
}
impl RotatingFile {
    pub fn open(path: &Path, limit: u64, total_files: u8) -> io::Result<Self> {
        if !path.is_absolute() || limit == 0 || total_files < 2 {
            return Err(io::Error::other("invalid journal path or rotation policy"));
        }
        if path
            .symlink_metadata()
            .is_ok_and(|meta| !meta.is_file() || meta.file_type().is_symlink())
        {
            return Err(io::Error::other("journal path must be a regular file"));
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let size = file.metadata()?.len();
        Ok(Self {
            path: path.to_path_buf(),
            file: Some(file),
            size,
            limit,
            archives: total_files - 1,
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
        match fs::remove_file(self.archive(self.archives)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        for index in (1..self.archives).rev() {
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
            return Err(io::Error::other("journal event exceeds file limit"));
        }
        if self.size.saturating_add(bytes.len() as u64) > self.limit {
            self.rotate()?;
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("journal unavailable after rotation error"))?;
        file.write_all(bytes)?;
        file.flush()?;
        self.size += bytes.len() as u64;
        Ok(())
    }
}
