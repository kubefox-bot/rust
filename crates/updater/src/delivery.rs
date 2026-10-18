use crate::Error;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub reference: String,
    pub digest: String,
    pub config_digest: String,
    pub platform: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub format_version: u8,
    pub product: String,
    pub channel: String,
    pub source_sha: String,
    pub build: String,
    pub sequence: u64,
    pub image: Image,
    pub release_manifest_sha256: String,
    pub updater_min: u64,
    pub updater_max: u64,
    pub schema_version: u64,
    pub protocol_min: u64,
    pub protocol_max: u64,
}
impl Descriptor {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > 65536 {
            return Err(Error::Verification);
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|_| Error::Verification)?;
        if value.format_version != 1
            || value.product != "astra-center"
            || value.channel != "main"
            || !hex(&value.source_sha, 40)
            || value.build != format!("git-{}", value.source_sha)
            || value.sequence == 0
            || value.sequence > 9_007_199_254_740_991
            || value.image.platform != "linux/amd64"
            || !digest(&value.image.digest)
            || !digest(&value.image.config_digest)
            || value.image.reference
                != format!("ghcr.io/kubefox-bot/astra-center@{}", value.image.digest)
            || !hex(&value.release_manifest_sha256, 64)
            || value.updater_min == 0
            || value.updater_min > 1
            || value.updater_max < 1
            || value.updater_min > value.updater_max
            || value.schema_version != 1
            || value.protocol_min == 0
            || value.protocol_min > 1
            || value.protocol_max < 1
            || value.protocol_min > value.protocol_max
        {
            return Err(Error::Verification);
        }
        Ok(value)
    }
}

pub fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|value| hex(value, 64))
}
