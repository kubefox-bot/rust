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
    for entry in
        dotenvy::from_path_iter(path).map_err(|_| ConfigError::EnvFile(path.to_path_buf()))?
    {
        let (key, value) = entry.map_err(|_| ConfigError::EnvFile(path.to_path_buf()))?;
        if KEYS.contains(&key.as_str()) && values.insert(key, value).is_some() {
            return Err(ConfigError::EnvFile(path.to_path_buf()));
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

/// Only an absent automatic file is optional; permission and parse errors are fatal.
pub(super) fn selected_file(
    explicit: Option<&Path>,
    executable: &Path,
) -> Result<(Values, Option<std::path::PathBuf>), ConfigError> {
    let path = match explicit {
        Some(path) => {
            std::path::absolute(path).map_err(|_| ConfigError::EnvFile(path.to_path_buf()))?
        }
        None => executable
            .parent()
            .ok_or(ConfigError::Setting("Не удалось определить каталог EXE"))?
            .join("cleaner.env"),
    };
    if explicit.is_none() {
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Values::new(), None))
            }
            Err(_) => return Err(ConfigError::EnvFile(path.to_path_buf())),
            Ok(_) => {}
        }
    }
    Ok((read_file(&path)?, Some(path)))
}
#[cfg(test)]
mod automatic_tests {
    use super::*;
    #[test]
    fn documented_windows_example_preserves_backslashes() {
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cleaner.env.example");
        let values = read_file(&example).unwrap();
        assert_eq!(values["HMAIL_DATA_DIR"], r"D:\hmailserver\data");
        assert_eq!(values["HMAIL_INI"], r"D:\hmailserver\Bin\hMailServer.ini");
    }
    #[test]
    fn only_sibling_cleaner_env_is_loaded_and_explicit_overrides_it() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("hmail-cleaner.exe");
        assert!(selected_file(None, &executable).unwrap().1.is_none());
        std::fs::write(dir.path().join(".env"), "HMAIL_SCAN=legacy").unwrap();
        assert!(selected_file(None, &executable).unwrap().0.is_empty());
        std::fs::write(dir.path().join("cleaner.env"), "HMAIL_SCAN=full").unwrap();
        assert_eq!(
            selected_file(None, &executable).unwrap().0["HMAIL_SCAN"],
            "full"
        );
        let other = dir.path().join("other.env");
        std::fs::write(&other, "HMAIL_SCAN=fast").unwrap();
        assert_eq!(
            selected_file(Some(&other), &executable).unwrap().0["HMAIL_SCAN"],
            "fast"
        );
        assert!(selected_file(Some(&dir.path().join("missing.env")), &executable).is_err());
        std::fs::write(dir.path().join("cleaner.env"), "HMAIL_SCAN='unterminated").unwrap();
        assert!(selected_file(None, &executable).is_err());
    }
}
