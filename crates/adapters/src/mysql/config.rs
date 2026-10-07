use sqlx::mysql::MySqlConnectOptions;
use std::{fmt, str::FromStr};
use thiserror::Error;

#[derive(Clone)]
pub struct DatabaseConfig {
    pub(super) options: MySqlConnectOptions,
}

#[derive(Debug, Error)]
#[error("DATABASE_URL must be a valid MySQL URL with a database name")]
pub struct DatabaseConfigError;

impl DatabaseConfig {
    pub fn parse(url: &str) -> Result<Self, DatabaseConfigError> {
        let options = MySqlConnectOptions::from_str(url).map_err(|_| DatabaseConfigError)?;
        if options.get_database().is_none_or(str::is_empty) {
            return Err(DatabaseConfigError);
        }
        Ok(Self { options })
    }
}

impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseConfig")
            .field("host", &self.options.get_host())
            .field("port", &self.options.get_port())
            .field("database", &self.options.get_database())
            .finish_non_exhaustive()
    }
}
