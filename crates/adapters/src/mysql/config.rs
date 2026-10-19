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
    pub fn from_parts(
        parts: &hmail_cleaner_protocol::DatabaseParts,
    ) -> Result<Self, DatabaseConfigError> {
        if !parts.permits_transport()
            || parts.host.trim().is_empty()
            || parts.port == 0
            || parts.username.trim().is_empty()
            || [
                parts.host.as_str(),
                parts.database.as_str(),
                parts.username.as_str(),
                parts.password.as_str(),
            ]
            .iter()
            .any(|part| part.contains('\0'))
        {
            return Err(DatabaseConfigError);
        }
        let ssl_mode = match parts.tls {
            hmail_cleaner_protocol::DatabaseTls::Preferred => sqlx::mysql::MySqlSslMode::Preferred,
            hmail_cleaner_protocol::DatabaseTls::Required => sqlx::mysql::MySqlSslMode::Required,
            hmail_cleaner_protocol::DatabaseTls::DisabledLocal => {
                sqlx::mysql::MySqlSslMode::Disabled
            }
        };
        Self::validated(
            MySqlConnectOptions::new()
                .host(&parts.host)
                .port(parts.port)
                .database(&parts.database)
                .username(&parts.username)
                .password(&parts.password)
                .ssl_mode(ssl_mode),
        )
    }
    pub fn parse(url: &str) -> Result<Self, DatabaseConfigError> {
        let options = MySqlConnectOptions::from_str(url).map_err(|_| DatabaseConfigError)?;
        Self::validated(options)
    }
    fn validated(options: MySqlConnectOptions) -> Result<Self, DatabaseConfigError> {
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

#[cfg(test)]
mod parts_tests {
    use super::*;
    use hmail_cleaner_protocol::DatabaseParts;
    #[test]
    fn explicit_parts_preserve_literal_username_database_and_ssl_policy() {
        let parts = DatabaseParts {
            host: "localhost".into(),
            port: 3306,
            database: "mail#/%db".into(),
            username: "user@:/?#".into(),
            password: "password@:/?#".to_owned().into(),
            tls: hmail_cleaner_protocol::DatabaseTls::Preferred,
        };
        let config = DatabaseConfig::from_parts(&parts).unwrap();
        assert_eq!(config.options.get_username(), "user@:/?#");
        assert_eq!(config.options.get_database(), Some("mail#/%db"));
        assert!(matches!(
            config.options.get_ssl_mode(),
            sqlx::mysql::MySqlSslMode::Preferred
        ));
        assert!(!format!("{config:?}").contains("password@"));
        assert!(DatabaseConfig::from_parts(&DatabaseParts {
            database: String::new(),
            ..parts
        })
        .is_err());
    }
}
