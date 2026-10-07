mod retention;
mod selection;
mod sources;
mod types;
use crate::args::Args;
use chrono::{Datelike, Local, NaiveDateTime, TimeZone};
use hmail_cleaner_adapters::mysql::DatabaseConfig;
use hmail_cleaner_core::{
    cleanup::{RunOptions, DEFAULT_BATCH_SIZE, DEFAULT_WORKERS, MAX_BATCH_SIZE, MAX_WORKERS},
    maintenance::{ScanMode, ScanOptions},
    selection::cutoff,
};
pub(crate) use retention::{before as parse_before, parse as parse_age};
use std::{num::NonZeroU32, path::PathBuf};
pub(crate) use types::{Config, ConfigError, RunIntent, ServicePolicy, Settings};
const DEFAULT_SERVICE_NAME: &str = "hMailServer";
pub(crate) fn load_settings(args: &Args) -> Result<Settings, ConfigError> {
    let system = sources::environment()?;
    let file = args
        .options()
        .env_file
        .as_deref()
        .map(sources::read_file)
        .transpose()?
        .unwrap_or_default();
    load_with_sources(args, &system, &file)
}
pub(crate) fn load_config(args: &Args) -> Result<Config, ConfigError> {
    let settings = load_settings(args)?;
    settings.for_run(
        RunIntent {
            mode: args.mode(),
            scan: settings.scan,
            service_policy: if args.options().manage_service {
                ServicePolicy::Manage
            } else {
                ServicePolicy::RequireStopped
            },
        },
        Local::now(),
    )
}
fn load_with_sources(
    args: &Args,
    system: &sources::Values,
    file: &sources::Values,
) -> Result<Settings, ConfigError> {
    let options = args.options();
    let value = |key| sources::value(key, system, file);
    let database =
        DatabaseConfig::parse(value(sources::DATABASE_KEY).ok_or(ConfigError::MissingDatabase)?)?;
    let data_dir = options
        .data_dir
        .clone()
        .or_else(|| value("HMAIL_DATA_DIR").map(PathBuf::from))
        .ok_or(ConfigError::Paths)?;
    let hmail_ini = options
        .hmail_ini
        .clone()
        .or_else(|| value("HMAIL_INI").map(PathBuf::from))
        .ok_or(ConfigError::Paths)?;
    if !data_dir.is_absolute() || !hmail_ini.is_absolute() {
        return Err(ConfigError::Paths);
    }
    let service_name = options
        .service_name
        .as_deref()
        .or_else(|| value("HMAIL_SERVICE_NAME"))
        .unwrap_or(DEFAULT_SERVICE_NAME);
    if service_name.trim().is_empty() {
        return Err(ConfigError::Setting("HMAIL_SERVICE_NAME"));
    }
    let workers = options
        .workers
        .map(Ok)
        .unwrap_or_else(|| {
            value("HMAIL_WORKERS")
                .map(str::parse)
                .transpose()
                .map(|v| v.unwrap_or(DEFAULT_WORKERS))
        })
        .map_err(|_| ConfigError::Setting("HMAIL_WORKERS"))?;
    if !(1..=MAX_WORKERS).contains(&workers) {
        return Err(ConfigError::Setting("HMAIL_WORKERS"));
    }
    let batch = options
        .batch_size
        .map(Ok)
        .unwrap_or_else(|| {
            value("HMAIL_BATCH_SIZE")
                .map(str::parse)
                .transpose()
                .map(|v| v.unwrap_or(DEFAULT_BATCH_SIZE))
        })
        .map_err(|_| ConfigError::Setting("HMAIL_BATCH_SIZE"))?;
    if !(1..=MAX_BATCH_SIZE).contains(&batch) {
        return Err(ConfigError::Setting("HMAIL_BATCH_SIZE"));
    }
    let scan = match options.scan {
        Some(crate::args::Scan::Fast) => ScanMode::Fast,
        Some(crate::args::Scan::Full) => ScanMode::Full,
        None => match value("HMAIL_SCAN").unwrap_or("fast") {
            "fast" => ScanMode::Fast,
            "full" => ScanMode::Full,
            _ => return Err(ConfigError::Setting("HMAIL_SCAN")),
        },
    };
    let log_file = options
        .log_file
        .clone()
        .or_else(|| value("HMAIL_LOG_FILE").map(PathBuf::from));
    if log_file.as_ref().is_some_and(|p| !p.is_absolute()) {
        return Err(ConfigError::Setting("HMAIL_LOG_FILE"));
    }
    Ok(Settings {
        database,
        data_dir,
        hmail_ini,
        service_name: service_name.to_string(),
        retention: selection::retention(options, system, file)?,
        scan,
        batch_size: NonZeroU32::new(batch).expect("validated positive batch"),
        workers,
        log_file,
    })
}
impl Settings {
    pub fn for_run(
        &self,
        intent: RunIntent,
        now: chrono::DateTime<Local>,
    ) -> Result<Config, ConfigError> {
        self.at(intent, now.naive_local())
    }
    fn at(&self, intent: RunIntent, now: NaiveDateTime) -> Result<Config, ConfigError> {
        let cutoff = cutoff(now, self.retention).map_err(|_| ConfigError::TimeRange)?;
        if !(1000..=9999).contains(&cutoff.year()) {
            return Err(ConfigError::TimeRange);
        }
        let scan = match intent.scan {
            ScanMode::Fast => ScanOptions::Fast,
            ScanMode::Full => ScanOptions::Full {
                file_cutoff: file_cutoff(&Local, cutoff)?,
            },
        };
        Ok(Config {
            mode: intent.mode,
            scan,
            service_policy: intent.service_policy,
            database: self.database.clone(),
            options: RunOptions {
                cutoff,
                batch_size: self.batch_size,
                workers: self.workers,
                data_directory: self.data_dir.clone(),
            },
            hmail_ini: self.hmail_ini.clone(),
            service_name: self.service_name.clone(),
        })
    }
    pub fn with_retention(&self, retention: hmail_cleaner_core::selection::Retention) -> Self {
        Self {
            retention,
            ..self.clone()
        }
    }
}
fn file_cutoff<T: TimeZone>(
    zone: &T,
    cutoff: NaiveDateTime,
) -> Result<std::time::SystemTime, ConfigError> {
    zone.from_local_datetime(&cutoff)
        .single()
        .map(std::time::SystemTime::from)
        .ok_or(ConfigError::LocalTime)
}
#[cfg(test)]
fn load_with_database(
    args: &Args,
    database: Option<&str>,
    now: NaiveDateTime,
) -> Result<Config, ConfigError> {
    let system = database
        .map(|v| sources::Values::from([(sources::DATABASE_KEY.into(), v.into())]))
        .unwrap_or_default();
    let file = args
        .options()
        .env_file
        .as_deref()
        .map(sources::read_file)
        .transpose()?
        .unwrap_or_default();
    let settings = load_with_sources(args, &system, &file)?;
    settings.at(
        RunIntent {
            mode: args.mode(),
            scan: settings.scan,
            service_policy: ServicePolicy::RequireStopped,
        },
        now,
    )
}
#[cfg(test)]
mod source_tests;
#[cfg(test)]
mod tests;
