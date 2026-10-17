use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};
const DEFAULT_ORIGIN: &str = "https://localhost:8443";
const DEFAULT_LISTEN: &str = "0.0.0.0:8443";
pub(crate) const AGENT_PACKAGES_DIRECTORY: &str = "/opt/hmail/agent-packages";
const CONFIG_FILE: &str = "center.json";
const MAX_CONFIG_BYTES: u64 = 8192;
const DEFAULT_STATIC_DIR: &str = "/opt/hmail/web";
pub struct CenterArgs {
    pub data_dir: PathBuf,
}
#[derive(Clone)]
pub struct CenterConfig {
    pub data_dir: PathBuf,
    pub listen: SocketAddr,
    pub origin: BrowserOrigin,
    pub static_dir: PathBuf,
}
#[derive(Clone)]
pub struct BrowserOrigin {
    canonical: String,
    authority: String,
    host: String,
}
impl BrowserOrigin {
    pub fn parse(value: &str) -> Result<Self, ConfigError> {
        let url = url::Url::parse(value).map_err(|_| ConfigError)?;
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(ConfigError);
        }
        let canonical = url.origin().ascii_serialization();
        let authority = canonical
            .strip_prefix("https://")
            .ok_or(ConfigError)?
            .to_owned();
        let host = url
            .host_str()
            .ok_or(ConfigError)?
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        Ok(Self {
            canonical,
            authority,
            host,
        })
    }
    pub fn as_str(&self) -> &str {
        &self.canonical
    }
    pub fn authority(&self) -> &str {
        &self.authority
    }
    pub fn host(&self) -> &str {
        &self.host
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    origin: String,
    listen: SocketAddr,
    #[serde(default = "default_static_dir")]
    static_dir: PathBuf,
}
fn default_static_dir() -> PathBuf {
    DEFAULT_STATIC_DIR.into()
}
#[derive(Debug, thiserror::Error)]
#[error("invalid center configuration")]
pub struct ConfigError;
impl CenterConfig {
    pub fn load(args: &CenterArgs) -> Result<Self, ConfigError> {
        if !args.data_dir.is_absolute() {
            return Err(ConfigError);
        }
        let file = args.data_dir.join(CONFIG_FILE);
        let wire = match file.symlink_metadata() {
            Ok(meta) if meta.is_file() && meta.len() <= MAX_CONFIG_BYTES => {
                serde_json::from_slice::<Wire>(&std::fs::read(file).map_err(|_| ConfigError)?)
                    .map_err(|_| ConfigError)?
            }
            Ok(_) => return Err(ConfigError),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Wire {
                origin: DEFAULT_ORIGIN.into(),
                listen: DEFAULT_LISTEN.parse().map_err(|_| ConfigError)?,
                static_dir: default_static_dir(),
            },
            Err(_) => return Err(ConfigError),
        };
        if wire.listen.port() == 0 || !wire.static_dir.is_absolute() {
            return Err(ConfigError);
        }
        Ok(Self {
            data_dir: args.data_dir.clone(),
            listen: wire.listen,
            origin: BrowserOrigin::parse(&wire.origin)?,
            static_dir: wire.static_dir,
        })
    }
    pub fn create(path: &Path, origin: &str, listen: SocketAddr) -> Result<(), ConfigError> {
        use std::io::Write;
        let origin = BrowserOrigin::parse(origin)?;
        if listen.port() == 0 {
            return Err(ConfigError);
        }
        let wire = Wire {
            origin: origin.canonical,
            listen,
            static_dir: default_static_dir(),
        };
        let bytes = serde_json::to_vec_pretty(&wire).map_err(|_| ConfigError)?;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path.join(CONFIG_FILE))
            .map_err(|_| ConfigError)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| ConfigError)
    }
}
