use super::{parse_age, parse_before, sources::Values, ConfigError};
use crate::args::Options;
use hmail_cleaner_core::selection::Retention;
pub(super) fn retention(
    options: &Options,
    system: &Values,
    file: &Values,
) -> Result<Retention, ConfigError> {
    match (options.older_than, options.before) {
        (Some(age), None) => return Ok(age),
        (None, Some(before)) => return Ok(Retention::Before(before)),
        (Some(_), Some(_)) => return Err(ConfigError::Retention),
        (None, None) => {}
    }
    for source in [system, file] {
        match (source.get("HMAIL_OLDER_THAN"), source.get("HMAIL_BEFORE")) {
            (Some(age), None) => return parse_age(age).map_err(|_| ConfigError::Retention),
            (None, Some(before)) => {
                return parse_before(before)
                    .map(Retention::Before)
                    .map_err(|_| ConfigError::Retention)
            }
            (Some(_), Some(_)) => return Err(ConfigError::Retention),
            (None, None) => {}
        }
    }
    Err(ConfigError::Retention)
}
