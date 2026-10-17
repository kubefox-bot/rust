use super::artifact;
use super::settings::AGENT_EXECUTABLE as EXECUTABLE;
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
const PACKAGE_FILES: &[&str] = &[
    EXECUTABLE,
    "hmail-cleaner-agent.exe.sha256",
    "manifest.json",
];
struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn publish(
    source: &Path,
    parent: &Path,
    manifest: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    fs::create_dir_all(parent)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let staged = Staging(parent.join(format!(".windows-x64-{}-{timestamp}", std::process::id())));
    fs::create_dir(&staged.0)?;
    artifact::publish(source, &staged.0, EXECUTABLE)?;
    manifest(&staged.0)?;
    for name in PACKAGE_FILES {
        if !staged.0.join(name).is_file() {
            return Err(io::Error::other("Неполный пакет агента"));
        }
    }
    let destination = parent.join("windows-x64");
    let previous = parent.join(".windows-x64.previous");
    if previous.exists() {
        return Err(io::Error::other(
            "Проверьте сохранённый предыдущий пакет агента",
        ));
    }
    let existing = match fs::symlink_metadata(&destination) {
        Ok(info) if !info.is_dir() || info.file_type().is_symlink() => {
            return Err(io::Error::other("Каталог пакета занят другим объектом"))
        }
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    if existing {
        fs::rename(&destination, &previous)?;
    }
    if let Err(error) = fs::rename(&staged.0, &destination) {
        if existing {
            fs::rename(&previous, &destination)?;
        }
        return Err(error);
    }
    if existing {
        for entry in fs::read_dir(&previous)? {
            let entry = entry?;
            if !entry.file_type()?.is_file()
                || !PACKAGE_FILES.iter().any(|name| entry.file_name() == *name)
            {
                return Err(io::Error::other(
                    "Предыдущий каталог содержит посторонние файлы и сохранён",
                ));
            }
        }
        fs::remove_dir_all(previous)?;
    }
    println!("Пакет агента: {}", destination.display());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_manifest_preserves_the_entire_previous_package() {
        let root = std::env::temp_dir().join(format!("hmail-agent-bundle-{}", std::process::id()));
        fs::create_dir_all(root.join("windows-x64")).unwrap();
        for name in PACKAGE_FILES {
            fs::write(root.join("windows-x64").join(name), b"previous").unwrap();
        }
        let source = root.join("source.exe");
        fs::write(&source, b"new executable").unwrap();
        assert!(publish(&source, &root, |_| Err(io::Error::other("manifest failed"))).is_err());
        for name in PACKAGE_FILES {
            assert_eq!(
                fs::read(root.join("windows-x64").join(name)).unwrap(),
                b"previous"
            );
        }
        publish(&source, &root, |stage| {
            fs::write(stage.join("manifest.json"), b"{}")
        })
        .unwrap();
        assert_eq!(
            fs::read(root.join("windows-x64").join(EXECUTABLE)).unwrap(),
            b"new executable"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
