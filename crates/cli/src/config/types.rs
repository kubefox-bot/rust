use crate::args::Mode;
use hmail_cleaner_adapters::mysql::{DatabaseConfig, DatabaseConfigError};
use hmail_cleaner_core::cleanup::RunOptions;
use std::path::PathBuf;
#[derive(Debug)]
pub(crate) struct Config {
    pub mode: Mode,
    pub scan: hmail_cleaner_core::maintenance::ScanOptions,
    pub database: DatabaseConfig,
    pub options: RunOptions,
    pub hmail_ini: PathBuf,
    pub service_name: String,
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    #[error("DATABASE_URL is required; set it in the environment or use --env-file")]
    MissingDatabase,
    #[error(transparent)]
    Database(#[from] DatabaseConfigError),
    #[error("DATABASE_URL is not valid Unicode")]
    InvalidEnvironment,
    #[error("cannot read or parse the explicit env file")]
    EnvFile,
    #[error("retention boundary must be a valid MySQL date between years 1000 and 9999")]
    TimeRange,
    #[error("DataFolder and hMailServer.ini paths must be absolute")]
    Paths,
    #[error("one retention option is required")]
    Retention,
    #[error("file cutoff is ambiguous or nonexistent in the server local timezone")]
    LocalTime,
}
