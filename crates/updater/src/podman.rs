use crate::{
    command,
    config::{private_read, Config},
    delivery::{digest, Descriptor},
    journal, Error,
};
use std::{ffi::OsString, path::Path, time::Duration};
const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
pub const DATA_VOLUME: &str = "hmail-center-data";
pub struct UnitIdentity {
    pub image: String,
    pub build: String,
}
pub fn unit_identity(unit: &str) -> Result<UnitIdentity, Error> {
    let images: Vec<_> = unit
        .lines()
        .filter_map(|line| line.strip_prefix("Image="))
        .collect();
    let checks: Vec<_> = unit
        .lines()
        .filter_map(|line| line.strip_prefix("HealthCmd="))
        .collect();
    if images.len() != 1 || checks.len() != 1 {
        return Err(Error::Invalid);
    }
    let image = images[0];
    let (repository, hash) = image.rsplit_once('@').ok_or(Error::Invalid)?;
    if repository.is_empty() || !digest(hash) || repository.bytes().any(|b| b.is_ascii_whitespace())
    {
        return Err(Error::Invalid);
    }
    let args: Vec<String> = serde_json::from_str(checks[0]).map_err(|_| Error::Invalid)?;
    if args.len() != 7
        || args[..6]
            != [
                "/usr/local/bin/hmail-cleaner",
                "center",
                "health",
                "--data-dir",
                "/data",
                "--expect-build",
            ]
        || !valid_build(&args[6])
    {
        return Err(Error::Invalid);
    }
    Ok(UnitIdentity {
        image: image.into(),
        build: args[6].clone(),
    })
}
fn valid_build(build: &str) -> bool {
    !build.is_empty()
        && build.len() <= 128
        && build
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".+_-".contains(&b))
}
pub fn candidate_unit(unit: &str, image: &str, build: &str) -> Result<String, Error> {
    unit_identity(unit)?;
    if !image
        .strip_prefix("ghcr.io/kubefox-bot/astra-center@")
        .is_some_and(digest)
        || !valid_build(build)
    {
        return Err(Error::Invalid);
    }
    let health = serde_json::to_string(&[
        "/usr/local/bin/hmail-cleaner",
        "center",
        "health",
        "--data-dir",
        "/data",
        "--expect-build",
        build,
    ])
    .map_err(|_| Error::Invalid)?;
    let lines: Vec<_> = unit
        .lines()
        .map(|line| {
            if line.starts_with("Image=") {
                format!("Image={image}")
            } else if line.starts_with("HealthCmd=") {
                format!("HealthCmd={health}")
            } else {
                line.to_owned()
            }
        })
        .collect();
    Ok(lines.join("\n") + "\n")
}
pub async fn podman(args: Vec<OsString>) -> Result<Vec<u8>, Error> {
    command::run(Path::new("/usr/bin/podman"), &args, COMMAND_TIMEOUT).await
}
pub async fn systemctl(action: &str) -> Result<(), Error> {
    if !matches!(action, "stop" | "start" | "daemon-reload") {
        return Err(Error::Invalid);
    }
    let mut args = vec!["--user".into(), action.into()];
    if action != "daemon-reload" {
        args.push("hmail-center.service".into())
    }
    command::run(Path::new("/usr/bin/systemctl"), &args, COMMAND_TIMEOUT).await?;
    Ok(())
}
pub async fn pull(config: &Config, descriptor: &Descriptor) -> Result<(), Error> {
    let _ = private_read(&config.registry_authfile, 16384)?;
    command::run(
        Path::new("/usr/bin/podman"),
        &[
            "pull".into(),
            "--authfile".into(),
            config.registry_authfile.as_os_str().into(),
            descriptor.image.reference.clone().into(),
        ],
        Duration::from_secs(900),
    )
    .await?;
    let bytes = podman(vec![
        "image".into(),
        "inspect".into(),
        descriptor.image.reference.clone().into(),
    ])
    .await?;
    let images: Vec<serde_json::Value> =
        serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?;
    let image = images.first().ok_or(Error::Invalid)?;
    if images.len() != 1
        || image["Architecture"] != "amd64"
        || image["Os"] != "linux"
        || image["Id"] != descriptor.image.config_digest
        || image["Labels"]["org.opencontainers.image.version"] != descriptor.build
    {
        return Err(Error::Verification);
    }
    let repo = image["RepoDigests"].as_array().ok_or(Error::Verification)?;
    if !repo
        .iter()
        .any(|value| value.as_str() == Some(descriptor.image.reference.as_str()))
    {
        return Err(Error::Verification);
    }
    Ok(())
}
pub async fn health(build: &str) -> Result<(), Error> {
    if !valid_build(build) {
        return Err(Error::Invalid);
    }
    podman(vec![
        "exec".into(),
        "hmail-center".into(),
        "/usr/local/bin/hmail-cleaner".into(),
        "center".into(),
        "health".into(),
        "--data-dir".into(),
        "/data".into(),
        "--expect-build".into(),
        build.into(),
    ])
    .await?;
    Ok(())
}
pub fn offline_args(image: &str) -> Vec<OsString> {
    vec![
        "run".into(),
        "--rm".into(),
        "--read-only".into(),
        "--user".into(),
        "10001:10001".into(),
        "--userns".into(),
        "keep-id:uid=10001,gid=10001".into(),
        "--volume".into(),
        format!("{DATA_VOLUME}:/data:Z,U").into(),
        image.into(),
    ]
}
pub async fn mountpoint() -> Result<std::path::PathBuf, Error> {
    let bytes = podman(vec![
        "volume".into(),
        "inspect".into(),
        "--format".into(),
        "{{.Mountpoint}}".into(),
        DATA_VOLUME.into(),
    ])
    .await?;
    let value = String::from_utf8(bytes).map_err(|_| Error::Invalid)?;
    let path = std::path::PathBuf::from(value.trim());
    if !path.is_absolute() {
        return Err(Error::Invalid);
    }
    Ok(path)
}
pub fn write_unit(path: &Path, unit: &str) -> Result<(), Error> {
    // Reuse the fsync/atomic writer without serializing the Quadlet as JSON.
    let parent = path.parent().ok_or(Error::Invalid)?;
    let temporary = parent.join(format!(".quadlet-{}", uuid::Uuid::new_v4()));
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| Error::Storage)?;
    file.write_all(unit.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|_| Error::Storage)?;
    std::fs::rename(&temporary, path).map_err(|_| Error::Storage)?;
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::Storage)
}
pub async fn mark(id: uuid::Uuid) -> Result<(), Error> {
    let directory = mountpoint().await?;
    journal::save(
        &directory.join("maintenance.json"),
        &serde_json::json!({"version":1,"id":id}),
    )
}
