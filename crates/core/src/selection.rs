use chrono::{Days, Months, NaiveDateTime};
use std::num::NonZeroU32;
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub enum Retention {
    Days(NonZeroU32),
    Months(NonZeroU32),
    Years(NonZeroU32),
    Before(NaiveDateTime),
}

#[derive(Debug, Error)]
#[error("retention date is outside the supported calendar")]
pub struct SelectionError;

const MONTHS_PER_YEAR: u32 = 12;

pub fn cutoff(now: NaiveDateTime, retention: Retention) -> Result<NaiveDateTime, SelectionError> {
    let result = match retention {
        Retention::Days(days) => now.checked_sub_days(Days::new(u64::from(days.get()))),
        Retention::Months(months) => now.checked_sub_months(Months::new(months.get())),
        Retention::Years(years) => years
            .get()
            .checked_mul(MONTHS_PER_YEAR)
            .and_then(|months| now.checked_sub_months(Months::new(months))),
        Retention::Before(boundary) => Some(boundary),
    };
    result.ok_or(SelectionError)
}
