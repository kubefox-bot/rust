use crate::{
    command,
    config::{private_read, Config},
    delivery::Descriptor,
    Error,
};
use ring::digest::{Context, SHA256};
use std::{ffi::OsString, io::Read, path::Path, time::Duration};
pub const COSIGN_SHA: &str = "4629c757b7618056f8ddd7e2625ae9fdd94c0372a65049520bc7d9df9efc7f71";
pub const TRUST_SHA: &str = "6494e21ea73fa7ee769f85f57d5a3e6a08725eae1e38c755fc3517c9e6bc0b66";
const SIGNER: &str =
    "https://github.com/kubefox-bot/rust/.github/workflows/center-delivery.yml@refs/heads/main";
const ISSUER: &str = "https://token.actions.githubusercontent.com";
pub fn hash_file(path: &Path) -> Result<String, Error> {
    let mut file = std::fs::File::open(path).map_err(|_| Error::Storage)?;
    let meta = file.metadata().map_err(|_| Error::Storage)?;
    if !meta.is_file() {
        return Err(Error::Invalid);
    }
    let mut context = Context::new(&SHA256);
    let mut bytes = [0u8; 65536];
    loop {
        let count = file.read(&mut bytes).map_err(|_| Error::Storage)?;
        if count == 0 {
            break;
        }
        context.update(&bytes[..count]);
    }
    Ok(context
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct VerifiedReceipt {
    version: u8,
    descriptor_sha256: String,
    bundle_sha256: String,
    trust_sha256: String,
}
pub struct VerifiedDelivery {
    descriptor: Descriptor,
}
impl VerifiedDelivery {
    pub fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
}
pub async fn verify(
    config: &Config,
    descriptor: &Path,
    bundle: &Path,
) -> Result<VerifiedDelivery, Error> {
    let descriptor_bytes = private_read(descriptor, 65536)?;
    let input = Descriptor::parse(&descriptor_bytes)?;
    let receipt = VerifiedReceipt {
        version: 1,
        descriptor_sha256: hash_file(descriptor)?,
        bundle_sha256: hash_file(bundle)?,
        trust_sha256: TRUST_SHA.into(),
    };
    let receipt_path = descriptor.with_extension("verified.json");
    if crate::journal::load::<VerifiedReceipt>(&receipt_path)?.as_ref() == Some(&receipt) {
        return Ok(VerifiedDelivery { descriptor: input });
    }
    use std::os::unix::fs::PermissionsExt;
    let binary = std::fs::symlink_metadata(&config.cosign).map_err(|_| Error::Verification)?;
    if !binary.is_file()
        || binary.permissions().mode() & 0o022 != 0
        || hash_file(&config.cosign)? != COSIGN_SHA
        || hash_file(&config.trust_root)? != TRUST_SHA
    {
        return Err(Error::Verification);
    }
    let _bundle = private_read(bundle, 2 * 1024 * 1024)?;
    let args: Vec<OsString> = vec![
        "verify-blob".into(),
        "--bundle".into(),
        bundle.as_os_str().into(),
        "--trusted-root".into(),
        config.trust_root.as_os_str().into(),
        "--certificate-identity".into(),
        SIGNER.into(),
        "--certificate-oidc-issuer".into(),
        ISSUER.into(),
        "--certificate-github-workflow-sha".into(),
        input.source_sha.clone().into(),
        descriptor.as_os_str().into(),
    ];
    command::run(&config.cosign, &args, Duration::from_secs(30))
        .await
        .map_err(|_| Error::Verification)?;
    if private_read(descriptor, 65536)? != descriptor_bytes {
        return Err(Error::Verification);
    }
    crate::journal::save(&receipt_path, &receipt)?;
    Ok(VerifiedDelivery { descriptor: input })
}
