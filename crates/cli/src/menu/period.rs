use crate::config::{parse_age, parse_before};
use hmail_cleaner_core::selection::Retention;
pub(super) fn parse(value: &str) -> Result<Retention, String> {
    parse_age(value).or_else(|_| parse_before(value).map(Retention::Before))
}
