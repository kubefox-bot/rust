use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};
pub struct CenterArgs {
    pub data_dir: PathBuf,
}
pub struct CenterConfig {
    pub data_dir: PathBuf,
    pub listen: SocketAddr,
}
#[derive(Debug, thiserror::Error)]
#[error("center data directory must be an absolute path")]
pub struct ConfigError;
impl CenterConfig {
    pub fn load(args: &CenterArgs) -> Result<Self, ConfigError> {
        if !args.data_dir.is_absolute() {
            return Err(ConfigError);
        }
        Ok(Self {
            data_dir: args.data_dir.clone(),
            listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080),
        })
    }
}
