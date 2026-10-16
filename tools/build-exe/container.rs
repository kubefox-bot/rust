use super::{
    run,
    settings::{Settings, BUILDER, RUST, TARGET},
};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};
fn docker(settings: &Settings) -> Command {
    let mut command = Command::new("docker");
    if let Some(context) = &settings.docker_context {
        command.args(["--context", context]);
    }
    command
}
struct Container {
    id: String,
    settings: Settings,
}
impl Drop for Container {
    fn drop(&mut self) {
        let _ = docker(&self.settings).args(["rm", "-f", &self.id]).output();
    }
}
fn copy_sources(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let kind = entry.file_type()?;
        if name == "target"
            || name == ".git"
            || name
                .to_str()
                .is_some_and(|n| n == ".env" || n.starts_with(".env.") || n.ends_with(".env"))
        {
            continue;
        }
        if kind.is_symlink() {
            return Err(io::Error::other(
                "Ссылка в исходниках сборки не поддерживается",
            ));
        }
        if kind.is_dir() {
            copy_sources(&path, &destination.join(name))?;
        } else {
            fs::copy(path, destination.join(name))?;
        }
    }
    Ok(())
}
fn available_runtime(settings: &Settings) -> io::Result<Settings> {
    if docker(settings)
        .args(["info", "--format", "{{.ServerVersion}}"])
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return Ok(settings.clone());
    }
    if settings.docker_context.is_none() {
        let project = Settings {
            docker_context: Some("colima-hmail-cleanup".into()),
            ..settings.clone()
        };
        if docker(&project)
            .args(["info", "--format", "{{.ServerVersion}}"])
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return Ok(project);
        }
    }
    Err(io::Error::other(
        "Docker недоступен: запустите Docker/Colima или задайте HMAIL_BUILD_DOCKER_CONTEXT",
    ))
}
pub(super) fn build(settings: &Settings) -> io::Result<PathBuf> {
    let runtime = available_runtime(settings)?;
    let settings = &runtime;
    let root = &settings.root;
    let staging = root.join("target/build-driver/source");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;
    for name in ["Cargo.toml", "Cargo.lock"] {
        fs::copy(root.join(name), staging.join(name))?;
    }
    copy_sources(&root.join("crates"), &staging.join("crates"))?;
    let revision = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()?;
    let revision = String::from_utf8_lossy(&revision.stdout).trim().to_string();
    let dirty = !Command::new("git")
        .args(["diff", "--quiet", "HEAD"])
        .status()?
        .success();
    let source_revision = format!("{revision}{}", if dirty { "-dirty" } else { "" });
    let script = format!("rustup toolchain install {RUST} --profile minimal --no-self-update && rustup target add --toolchain {RUST} {TARGET} && cargo +{RUST} xwin build --release -p hmail-cleaner --target {TARGET} --locked");
    let created = docker(settings)
        .args([
            "create",
            "--workdir",
            "/work",
            "--mount",
            "type=volume,source=hmail-cleaner-build-cache,target=/cache",
            "--env",
            "CARGO_HOME=/cache/cargo",
            "--env",
            "RUSTUP_HOME=/cache/rustup",
            "--env",
            "CARGO_TARGET_DIR=/cache/target",
            "--env",
            "XWIN_CACHE_DIR=/cache/xwin",
            "--env",
            "RUSTFLAGS=-C target-feature=+crt-static",
            "--env",
            &format!("HMAIL_SOURCE_REVISION={source_revision}"),
            "--entrypoint",
            "sh",
            BUILDER,
            "-c",
            &script,
        ])
        .output()?;
    if !created.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&created.stderr).into_owned(),
        ));
    }
    let container = Container {
        id: String::from_utf8_lossy(&created.stdout).trim().to_owned(),
        settings: settings.clone(),
    };
    run(docker(settings)
        .arg("cp")
        .arg(staging.join("."))
        .arg(format!("{}:/work", container.id)))?;
    run(docker(settings).args(["start", "--attach", &container.id]))?;
    let output = root.join("target/build-driver/hmail-cleaner.exe");
    run(docker(settings)
        .arg("cp")
        .arg(format!(
            "{}:/cache/target/{TARGET}/release/hmail-cleaner.exe",
            container.id
        ))
        .arg(&output))?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_transfer_omits_env_files_and_build_output() {
        let root = std::env::temp_dir().join(format!("hmail-transfer-test-{}", std::process::id()));
        let source = root.join("source");
        let destination = root.join("destination");
        fs::create_dir_all(source.join("target")).unwrap();
        fs::write(source.join("private.env"), "secret-marker").unwrap();
        fs::write(source.join(".env"), "secret-marker").unwrap();
        fs::write(source.join(".env.production"), "secret-marker").unwrap();
        fs::write(source.join("module.rs"), "source").unwrap();
        fs::write(source.join("target/generated"), "artifact").unwrap();
        copy_sources(&source, &destination).unwrap();
        assert!(destination.join("module.rs").exists());
        assert!(!destination.join("private.env").exists());
        assert!(!destination.join(".env").exists());
        assert!(!destination.join(".env.production").exists());
        assert!(!destination.join("target").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
