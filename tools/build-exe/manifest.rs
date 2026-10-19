use std::{io, path::Path, process::Command};
pub(super) fn version(root: &Path) -> io::Result<String> {
    let output = Command::new("bun")
        .args(["tools/release/agent-manifest.ts", "version"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("Bun не смог определить версию пакета"));
    }
    let version = String::from_utf8(output.stdout)
        .map_err(|_| io::Error::other("Некорректная версия"))?
        .trim()
        .to_owned();
    if version.is_empty()
        || version.len() > 128
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".+_-".contains(&byte))
    {
        return Err(io::Error::other("Некорректная версия пакета"));
    }
    Ok(version)
}
pub(super) fn write(root: &Path, directory: &Path, version: &str) -> io::Result<()> {
    let status = Command::new("bun")
        .args(["tools/release/agent-manifest.ts", "write", "--directory"])
        .arg(directory)
        .args(["--build", version])
        .current_dir(root)
        .status()?;
    if !status.success() {
        return Err(io::Error::other("Не удалось собрать manifest агента"));
    }
    Ok(())
}
