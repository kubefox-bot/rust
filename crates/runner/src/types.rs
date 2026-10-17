use hmail_cleaner_adapters::mysql::DatabaseConfig;
use hmail_cleaner_core::{
    cleanup::RunOptions,
    maintenance::{recursive::Action, ScanOptions},
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Preview,
    Clean,
}

impl Mode {
    pub fn action(self) -> Action {
        match self {
            Self::Preview => Action::Preview,
            Self::Clean => Action::Clean,
        }
    }
    pub(crate) fn from_action(action: Action) -> Self {
        match action {
            Action::Preview => Self::Preview,
            Action::Clean => Self::Clean,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServicePolicy {
    RequireStopped,
    Manage,
}

pub struct RunRequest {
    pub database: DatabaseConfig,
    pub options: RunOptions,
    pub scan: ScanOptions,
    pub action: Action,
    pub service_policy: ServicePolicy,
    pub hmail_ini: PathBuf,
    pub service_name: String,
}
