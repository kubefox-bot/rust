use super::ConfigError;
use std::{collections::BTreeMap, path::Path};
pub(super) const DATABASE_KEY: &str = "DATABASE_URL";
const KEYS: &[&str] = &[
    DATABASE_KEY,
    "HMAIL_DATA_DIR",
    "HMAIL_INI",
    "HMAIL_OLDER_THAN",
    "HMAIL_BEFORE",
    "HMAIL_SERVICE_NAME",
    "HMAIL_BATCH_SIZE",
    "HMAIL_WORKERS",
    "HMAIL_SCAN",
    "HMAIL_LOG_FILE",
    "HMAIL_RUN_AT",
];
pub(super) type Values = BTreeMap<String, String>;
pub(super) fn environment() -> Result<Values, ConfigError> {
    KEYS.iter()
        .filter_map(|key| match std::env::var(key) {
            Ok(value) => Some(Ok((key.to_string(), value))),
            Err(std::env::VarError::NotPresent) => None,
            Err(_) => Some(Err(ConfigError::InvalidEnvironment)),
        })
        .collect()
}
pub(super) fn read_file(path: &Path) -> Result<Values, ConfigError> {
    let mut values = Values::new();
    for entry in dotenvy::from_path_iter(path).map_err(|_| ConfigError::EnvFile)? {
        let (key, value) = entry.map_err(|_| ConfigError::EnvFile)?;
        if KEYS.contains(&key.as_str()) && values.insert(key, value).is_some() {
            return Err(ConfigError::EnvFile);
        }
    }
    Ok(values)
}
pub(super) fn value<'a>(key: &str, system: &'a Values, file: &'a Values) -> Option<&'a str> {
    system
        .get(key)
        .or_else(|| file.get(key))
        .map(String::as_str)
}
