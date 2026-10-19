use crate::{
    config::{private_directory, Config},
    delivery::Descriptor,
    journal::{self, Status},
    network::Network,
    verification::{self, VerifiedDelivery},
    Error,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
#[derive(Deserialize)]
pub struct Release {
    pub id: u64,
    pub tag_name: String,
    pub draft: bool,
    pub prerelease: bool,
    pub assets: Vec<Asset>,
}
#[derive(Deserialize)]
pub struct Asset {
    pub id: u64,
    pub name: String,
    pub url: String,
    pub size: u64,
}
impl Release {
    fn asset(&self, name: &str) -> Result<&Asset, Error> {
        let mut found = self.assets.iter().filter(|a| a.name == name);
        let value = found.next().ok_or(Error::Invalid)?;
        if found.next().is_some() {
            return Err(Error::Invalid);
        }
        Ok(value)
    }
}
pub struct Candidate {
    pub delivery: VerifiedDelivery,
    pub release: Release,
    pub directory: PathBuf,
}
pub fn eligible(descriptor: &Descriptor, status: &Status) -> Result<bool, Error> {
    if descriptor.sequence == status.installed_sequence
        && (status
            .installed_build
            .as_ref()
            .is_some_and(|build| build != &descriptor.build)
            || status
                .installed_digest
                .as_ref()
                .is_some_and(|digest| digest != &descriptor.image.digest))
    {
        return Err(Error::Verification);
    }
    Ok(descriptor.sequence > status.installed_sequence
        && status.quarantined_digest.as_ref() != Some(&descriptor.image.digest))
}
pub async fn discover(
    config: &Config,
    network: &Network,
    status: &Status,
) -> Result<Option<Candidate>, Error> {
    let mut selected: Option<Candidate> = None;
    let mut exhausted = false;
    for page in 1..=10 {
        let url = format!(
            "https://api.github.com/repos/{}/releases?per_page=30&page={page}",
            config.repository
        );
        let bytes = network.bytes(&url, 2 * 1024 * 1024, false).await?;
        let releases: Vec<Release> = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?;
        let end = releases.len() < 30;
        for release in releases {
            if release.draft || release.prerelease || !release.tag_name.starts_with("center-") {
                continue;
            }
            let descriptor = release.asset("delivery.json")?;
            let bundle = release.asset("delivery.sigstore.json")?;
            if descriptor.size > 65536 || bundle.size > 2 * 1024 * 1024 {
                return Err(Error::Invalid);
            }
            let directory = config.staging_dir.join(format!("release-{}", release.id));
            private_directory(&directory)?;
            let descriptor_path = directory.join("delivery.json");
            let bundle_path = directory.join("delivery.sigstore.json");
            let receipt_path = directory.join("asset-identity.json");
            let identity = serde_json::json!({"descriptor_id":descriptor.id,"bundle_id":bundle.id});
            let cached = journal::load::<serde_json::Value>(&receipt_path)?;
            if cached.as_ref() != Some(&identity)
                || !descriptor_path.try_exists().map_err(|_| Error::Storage)?
                || !bundle_path.try_exists().map_err(|_| Error::Storage)?
            {
                download_metadata(network, &descriptor.url, &descriptor_path, 65536).await?;
                download_metadata(network, &bundle.url, &bundle_path, 2 * 1024 * 1024).await?;
                journal::save(&receipt_path, &identity)?;
            }
            let delivery = verification::verify(config, &descriptor_path, &bundle_path).await?;
            let value = delivery.descriptor();
            if release.tag_name != format!("center-{}", value.source_sha) {
                return Err(Error::Verification);
            }
            if eligible(value, status)?
                && selected
                    .as_ref()
                    .is_none_or(|previous| value.sequence > previous.delivery.descriptor().sequence)
            {
                selected = Some(Candidate {
                    delivery,
                    release,
                    directory,
                });
            }
        }
        if end {
            exhausted = true;
            break;
        }
    }
    if !exhausted {
        return Err(Error::Invalid);
    } // Do not assume the first page is chronologically authoritative.
    Ok(selected)
}
async fn download_metadata(
    network: &Network,
    url: &str,
    path: &Path,
    limit: u64,
) -> Result<(), Error> {
    let temporary = path.with_extension("download");
    if temporary.try_exists().map_err(|_| Error::Storage)? {
        std::fs::remove_file(&temporary).map_err(|_| Error::Storage)?;
    }
    network.file(url, &temporary, limit).await?;
    std::fs::rename(&temporary, path).map_err(|_| Error::Storage)?;
    std::fs::File::open(path.parent().ok_or(Error::Invalid)?)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::Storage)
}
pub async fn manifest(
    candidate: &Candidate,
    network: &Network,
) -> Result<serde_json::Value, Error> {
    let asset = candidate.release.asset("release-manifest.json")?;
    let path = candidate.directory.join("release-manifest.json");
    download_metadata(network, &asset.url, &path, 1024 * 1024).await?;
    if verification::hash_file(&path)? != candidate.delivery.descriptor().release_manifest_sha256 {
        return Err(Error::Verification);
    }
    let value = journal::load::<serde_json::Value>(&path)?.ok_or(Error::Invalid)?;
    let descriptor = candidate.delivery.descriptor();
    if value["build"] != descriptor.build
        || value["source_revision"] != descriptor.source_sha[..12]
        || value["dirty"] != false
        || value["image"]["reference"] != descriptor.image.reference
        || value["image"]["digest"] != descriptor.image.digest
        || value["image"]["config_digest"] != descriptor.image.config_digest
    {
        return Err(Error::Verification);
    }
    Ok(value)
}
