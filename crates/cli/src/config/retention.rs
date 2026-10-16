use chrono::{NaiveDate, NaiveDateTime};
use hmail_cleaner_core::selection::Retention;
use std::num::NonZeroU32;
const DATE_FORMAT: &str = "%Y-%m-%d";
const TIME_FORMATS: &[&str] = &["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"];
pub(crate) fn parse(value: &str) -> Result<Retention, String> {
    let (count, unit) = if let Some(count) = value.strip_suffix("mo") {
        (count, "mo")
    } else if let Some(count) = value.strip_suffix('d') {
        (count, "d")
    } else if let Some(count) = value.strip_suffix('y') {
        (count, "y")
    } else {
        return Err("Укажите положительный период: 30d, 1mo или 1y".into());
    };
    if !count.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Период должен быть положительным целым числом".into());
    }
    let count = count
        .parse::<u32>()
        .ok()
        .and_then(NonZeroU32::new)
        .ok_or("Период должен быть положительным целым числом в диапазоне u32")?;
    match unit {
        "d" => Ok(Retention::Days(count)),
        "mo" => Ok(Retention::Months(count)),
        "y" => Ok(Retention::Years(count)),
        _ => unreachable!("validated age suffix"),
    }
}
pub(crate) fn before(value: &str) -> Result<NaiveDateTime, String> {
    if let Ok(date) = NaiveDate::parse_from_str(value, DATE_FORMAT) {
        return date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| "Некорректная местная дата".into());
    }
    TIME_FORMATS
        .iter()
        .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
        .ok_or_else(|| {
            "Укажите YYYY-MM-DD или YYYY-MM-DDTHH:MM:SS в местном времени сервера".into()
        })
}
