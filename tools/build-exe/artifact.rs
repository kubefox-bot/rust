use std::{fs, io, path::Path, process::Command};
fn hash(path: &Path) -> io::Result<String> {
    let output = if cfg!(windows) {
        Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-FileHash -Algorithm SHA256 -LiteralPath $env:HMAIL_HASH_INPUT).Hash",
            ])
            .env("HMAIL_HASH_INPUT", path)
            .output()?
    } else {
        Command::new("shasum")
            .args(["-a", "256"])
            .arg(path)
            .output()?
    };
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "Не удалось вычислить SHA-256 ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let digest = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::Error::other("Некорректный результат SHA-256"));
    }
    Ok(digest)
}
pub(super) fn publish(source: &Path, root: &Path, name: &str) -> io::Result<()> {
    let executable = root.join(name);
    let checksum = root.join(format!("{name}.sha256"));
    let staged = root.join(format!(".{name}.new"));
    let staged_sum = root.join(format!(".{name}.sha256.new"));
    for destination in [&executable, &checksum] {
        match fs::symlink_metadata(destination) {
            Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => {
                return Err(io::Error::other(
                    "Путь артефакта занят каталогом или ссылкой",
                ))
            }
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    fs::copy(source, &staged)?;
    fs::write(&staged_sum, format!("{}  {name}\n", hash(&staged)?))?;
    let backup = root.join(format!(".{name}.previous"));
    let backup_sum = root.join(format!(".{name}.sha256.previous"));
    if backup.exists() || backup_sum.exists() {
        return Err(io::Error::other(
            "Найдена предыдущая резервная копия сборки; проверьте .hmail-cleaner.exe*.previous",
        ));
    }
    let had_exe = executable.exists();
    let had_sum = checksum.exists();
    if had_exe {
        fs::rename(&executable, &backup)?;
    }
    let result = (|| {
        if had_sum {
            fs::rename(&checksum, &backup_sum)?;
        }
        fs::rename(&staged, &executable)?;
        fs::rename(&staged_sum, &checksum)
    })();
    if let Err(error) = result {
        if had_exe && backup.exists() {
            let _ = fs::remove_file(&executable);
            fs::rename(&backup, &executable)?;
        } else if !had_exe {
            let _ = fs::remove_file(&executable);
        }
        if had_sum && backup_sum.exists() {
            let _ = fs::remove_file(&checksum);
            fs::rename(&backup_sum, &checksum)?;
        }
        return Err(error);
    }
    if had_exe {
        fs::remove_file(backup)?;
    }
    if had_sum {
        fs::remove_file(backup_sum)?;
    }
    println!(
        "Готово: {}\nSHA-256: {}",
        executable.display(),
        hash(&executable)?
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "hmail-build-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn hashes_literal_unicode_paths_with_shell_metacharacters() {
        let dir = Directory::new();
        let source = dir.0.join("письмо ' $ & [x].bin");
        fs::write(&source, b"abc").unwrap();
        assert_eq!(
            hash(&source).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn publishes_matching_pair_and_preserves_previous_pair_on_failure() {
        let dir = Directory::new();
        let root = &dir.0;
        let source = root.join("source.exe");
        fs::write(&source, b"new executable").unwrap();
        fs::write(root.join("hmail-cleaner.exe"), b"old executable").unwrap();
        fs::write(root.join("hmail-cleaner.exe.sha256"), b"old checksum").unwrap();
        assert!(publish(&root.join("missing.exe"), root, "hmail-cleaner.exe").is_err());
        assert_eq!(
            fs::read(root.join("hmail-cleaner.exe")).unwrap(),
            b"old executable"
        );
        assert_eq!(
            fs::read(root.join("hmail-cleaner.exe.sha256")).unwrap(),
            b"old checksum"
        );
        publish(&source, root, "hmail-cleaner.exe").unwrap();
        assert_eq!(
            fs::read(root.join("hmail-cleaner.exe")).unwrap(),
            b"new executable"
        );
        assert_eq!(
            fs::read_to_string(root.join("hmail-cleaner.exe.sha256")).unwrap(),
            format!("{}  hmail-cleaner.exe\n", hash(&source).unwrap())
        );
        assert!(!root.join(".hmail-cleaner.exe.previous").exists());
    }
}

#[cfg(test)]
mod agent_tests {
    use super::*;
    #[test]
    fn agent_publication_is_separate_from_the_manual_cli_pair() {
        let root = std::env::temp_dir().join(format!("hmail-agent-publish-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let package = root.join("dist/agent/windows-x64");
        fs::create_dir_all(&package).unwrap();
        fs::write(root.join("hmail-cleaner.exe"), b"existing CLI").unwrap();
        let source = root.join("source.exe");
        fs::write(&source, b"fixture agent").unwrap();
        publish(&source, &package, "hmail-cleaner-agent.exe").unwrap();
        assert_eq!(
            fs::read(package.join("hmail-cleaner-agent.exe")).unwrap(),
            b"fixture agent"
        );
        assert_eq!(
            fs::read(root.join("hmail-cleaner.exe")).unwrap(),
            b"existing CLI"
        );
        assert_eq!(
            fs::read_to_string(package.join("hmail-cleaner-agent.exe.sha256")).unwrap(),
            format!("{}  hmail-cleaner-agent.exe\n", hash(&source).unwrap())
        );
        fs::remove_dir_all(root).unwrap();
    }
}
