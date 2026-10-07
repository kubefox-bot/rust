mod types;
use crate::args::Args;
use chrono::{Datelike, Local, NaiveDateTime};
use hmail_cleaner_adapters::mysql::DatabaseConfig;
use hmail_cleaner_core::{
    cleanup::RunOptions,
    selection::{cutoff, Retention},
};
use std::{
    env::{self, VarError},
    num::NonZeroU32,
};
pub(crate) use types::{Config, ConfigError};
const DATABASE_KEY: &str = "DATABASE_URL";

pub(crate) fn load_config(args: &Args) -> Result<Config, ConfigError> {
    let database = match env::var(DATABASE_KEY) {
        Ok(value) => Some(value),
        Err(VarError::NotPresent) => None,
        Err(VarError::NotUnicode(_)) => return Err(ConfigError::InvalidEnvironment),
    };
    load_with_database(args, database.as_deref(), Local::now().naive_local())
}
fn load_with_database(
    args: &Args,
    database: Option<&str>,
    now: NaiveDateTime,
) -> Result<Config, ConfigError> {
    let options = args.options();
    let from_file = options
        .env_file
        .as_deref()
        .map(read_database)
        .transpose()?
        .flatten();
    let url = database
        .or(from_file.as_deref())
        .ok_or(ConfigError::MissingDatabase)?;
    let database = DatabaseConfig::parse(url)?;
    let retention = match (options.older_than, options.before) {
        (Some(age), None) => age,
        (None, Some(date)) => Retention::Before(date),
        _ => return Err(ConfigError::Retention),
    };
    let cutoff = cutoff(now, retention).map_err(|_| ConfigError::TimeRange)?;
    if !(1000..=9999).contains(&cutoff.year()) {
        return Err(ConfigError::TimeRange);
    }
    if !options.data_dir.is_absolute() || !options.hmail_ini.is_absolute() {
        return Err(ConfigError::Paths);
    }
    Ok(Config {
        mode: args.mode(),
        database,
        options: RunOptions {
            cutoff,
            batch_size: NonZeroU32::new(options.batch_size).expect("Clap rejects zero batch size"),
            workers: options.workers,
            data_directory: options.data_dir.clone(),
        },
        hmail_ini: options.hmail_ini.clone(),
        service_name: options.service_name.clone(),
    })
}
fn read_database(path: &std::path::Path) -> Result<Option<String>, ConfigError> {
    let values = dotenvy::from_path_iter(path).map_err(|_| ConfigError::EnvFile)?;
    let mut database = None;
    for value in values {
        let (key, value) = value.map_err(|_| ConfigError::EnvFile)?;
        if key == DATABASE_KEY {
            if database.is_some() {
                return Err(ConfigError::EnvFile);
            }
            database = Some(value);
        }
    }
    Ok(database)
}
#[cfg(test)]
mod tests;
