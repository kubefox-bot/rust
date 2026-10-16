use crate::args::Mode;
use hmail_cleaner_adapters::mysql::{DatabaseConfig, DatabaseConfigError};
use hmail_cleaner_core::cleanup::RunOptions;
use hmail_cleaner_core::{maintenance::ScanMode, selection::Retention};
use std::num::NonZeroU32;
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServicePolicy {
    RequireStopped,
    Manage,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunIntent {
    pub mode: Mode,
    pub scan: ScanMode,
    pub service_policy: ServicePolicy,
}
#[derive(Debug, Clone)]
pub(crate) struct Settings {
    pub env_file: Option<PathBuf>,
    pub database: DatabaseConfig,
    pub data_dir: PathBuf,
    pub hmail_ini: PathBuf,
    pub service_name: String,
    pub retention: Retention,
    pub scan: ScanMode,
    pub batch_size: NonZeroU32,
    pub workers: u8,
    pub log_file: Option<PathBuf>,
}
#[derive(Debug, Clone)]
pub(crate) struct ServiceSettings {
    pub cleanup: Settings,
    pub run_at: chrono::NaiveTime,
}
#[derive(Debug)]
pub(crate) struct Config {
    pub env_file: Option<PathBuf>,
    pub mode: Mode,
    pub service_policy: ServicePolicy,
    pub scan: hmail_cleaner_core::maintenance::ScanOptions,
    pub database: DatabaseConfig,
    pub options: RunOptions,
    pub hmail_ini: PathBuf,
    pub service_name: String,
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    #[error("Не задан DATABASE_URL: укажите его в cleaner.env рядом с EXE, окружении или через --env-file")]
    MissingDatabase,
    #[error(transparent)]
    Database(#[from] DatabaseConfigError),
    #[error("Переменная окружения содержит некорректный Unicode")]
    InvalidEnvironment,
    #[error("Не удалось прочитать или разобрать файл настроек env: {0}")]
    EnvFile(PathBuf),
    #[error("Граница даты должна быть допустимой датой MySQL между 1000 и 9999 годом")]
    TimeRange,
    #[error("Пути DataFolder и hMailServer.ini должны быть абсолютными")]
    Paths,
    #[error("Задайте период хранения или точную границу даты")]
    Retention,
    #[error("Граница даты неоднозначна или не существует в часовом поясе сервера")]
    LocalTime,
    #[error("Не задана или некорректна настройка {0}")]
    Setting(&'static str),
}
