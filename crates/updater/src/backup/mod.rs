use crate::{
    config::private_directory,
    journal::{self, Transaction},
    podman,
    verification::hash_file,
    Error,
};
use std::{ffi::OsString, path::Path, time::Duration};
pub mod restore;
/// Snapshot is created by the OLD executable with no live center process.
pub async fn create(tx: &Transaction) -> Result<(), Error> {
    let parent = tx.backup.parent().ok_or(Error::Invalid)?;
    private_directory(parent)?;
    let mut args = podman::offline_args(&tx.previous_image);
    let image = args.pop().ok_or(Error::Invalid)?;
    args.extend([
        "--volume".into(),
        format!("{}:/backup:Z", parent.display()).into(),
        image,
        "center".into(),
        "backup".into(),
        "--data-dir".into(),
        "/data".into(),
        "--destination".into(),
        "/backup/snapshot".into(),
    ]);
    podman::podman(args).await?;
    verify(&tx.backup, &tx.previous_build)?;
    let names = [
        "center.sqlite",
        "https-cert.pem",
        "https-key.pem",
        "ca-cert.pem",
        "center.json",
        "manifest.json",
    ];
    let mut hashes = std::collections::BTreeMap::new();
    for name in names {
        hashes.insert(name.to_owned(), hash_file(&tx.backup.join(name))?);
    }
    if tx
        .backup
        .join("update-policy.json")
        .try_exists()
        .map_err(|_| Error::Storage)?
    {
        hashes.insert(
            "update-policy.json".into(),
            hash_file(&tx.backup.join("update-policy.json"))?,
        );
    }
    journal::save(&parent.join("snapshot-hashes.json"), &hashes)?;
    Ok(())
}
pub fn verify(path: &Path, build: &str) -> Result<(), Error> {
    let manifest: serde_json::Value =
        journal::load(&path.join("manifest.json"))?.ok_or(Error::Verification)?;
    if manifest["format_version"] != 1
        || manifest["vault_id"]
            .as_str()
            .and_then(|value| uuid::Uuid::parse_str(value).ok())
            .is_none()
        || manifest["schema_version"] != 1
        || manifest["build_version"] != build
        || manifest["vault_id"].as_str().is_none()
    {
        return Err(Error::Verification);
    }
    for name in [
        "center.sqlite",
        "https-cert.pem",
        "https-key.pem",
        "ca-cert.pem",
        "center.json",
    ] {
        let meta = std::fs::symlink_metadata(path.join(name)).map_err(|_| Error::Storage)?;
        if !meta.is_file() || meta.len() == 0 {
            return Err(Error::Verification);
        }
    }
    Ok(())
}
pub fn verify_hashes(tx: &Transaction) -> Result<(), Error> {
    verify(&tx.backup, &tx.previous_build)?;
    let hashes: std::collections::BTreeMap<String, String> = journal::load(
        &tx.backup
            .parent()
            .ok_or(Error::Invalid)?
            .join("snapshot-hashes.json"),
    )?
    .ok_or(Error::Verification)?;
    let required = [
        "center.sqlite",
        "https-cert.pem",
        "https-key.pem",
        "ca-cert.pem",
        "center.json",
        "manifest.json",
    ];
    if required.iter().any(|name| !hashes.contains_key(*name))
        || hashes
            .keys()
            .any(|name| !required.contains(&name.as_str()) && name != "update-policy.json")
    {
        return Err(Error::Verification);
    }
    for entry in std::fs::read_dir(&tx.backup).map_err(|_| Error::Storage)? {
        let entry = entry.map_err(|_| Error::Storage)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::Verification)?;
        if !entry.file_type().map_err(|_| Error::Storage)?.is_file() || !hashes.contains_key(&name)
        {
            return Err(Error::Verification);
        }
    }
    for (name, hash) in hashes {
        if name.contains('/') || name.contains('\\') || name.contains("..") {
            return Err(Error::Verification);
        }
        if hash_file(&tx.backup.join(name))? != hash {
            return Err(Error::Verification);
        }
    }
    Ok(())
}
pub async fn export_candidate(tx: &Transaction) -> Result<(), Error> {
    if tx
        .candidate_export
        .try_exists()
        .map_err(|_| Error::Storage)?
    {
        let expected: String = journal::load(&tx.candidate_export.with_extension("sha256.json"))?
            .ok_or(Error::Verification)?;
        return if hash_file(&tx.candidate_export)? == expected {
            Ok(())
        } else {
            Err(Error::Verification)
        };
    }
    let temporary = tx.candidate_export.with_extension("partial");
    if temporary.try_exists().map_err(|_| Error::Storage)? {
        std::fs::remove_file(&temporary).map_err(|_| Error::Storage)?;
    }
    podman::podman(vec![
        "volume".into(),
        "export".into(),
        podman::DATA_VOLUME.into(),
        "--output".into(),
        temporary.as_os_str().into(),
    ])
    .await?;
    std::fs::File::open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(|_| Error::Storage)?;
    let hash = hash_file(&temporary)?;
    journal::save(&tx.candidate_export.with_extension("sha256.json"), &hash)?;
    std::fs::rename(&temporary, &tx.candidate_export).map_err(|_| Error::Storage)?;
    std::fs::File::open(tx.candidate_export.parent().ok_or(Error::Invalid)?)
        .and_then(|file| file.sync_all())
        .map_err(|_| Error::Storage)
}
pub async fn assert_identity(tx: &Transaction) -> Result<(), Error> {
    let directory = podman::mountpoint().await?;
    for name in [
        "https-cert.pem",
        "ca-cert.pem",
        "https-key.pem",
        "center.json",
    ] {
        if hash_file(&directory.join(name))? != hash_file(&tx.backup.join(name))? {
            return Err(Error::Verification);
        }
    }
    Ok(())
}
pub async fn filesystem_copy(source: &Path, destination: &Path) -> Result<(), Error> {
    crate::command::run(
        Path::new("/usr/bin/cp"),
        &[
            OsString::from("-a"),
            source.as_os_str().into(),
            destination.as_os_str().into(),
        ],
        Duration::from_secs(120),
    )
    .await?;
    Ok(())
}
