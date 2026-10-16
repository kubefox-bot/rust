use crate::config::{parse_age, parse_before};
use hmail_cleaner_core::selection::Retention;
pub(super) fn parse(value: &str) -> Result<Retention, String> {
    parse_age(value).or_else(|_| parse_before(value).map(Retention::Before))
}

pub(super) fn description(retention: Retention) -> String {
    match retention {
        Retention::Days(n) => format!("старше {} дней", n.get()),
        Retention::Months(n) => format!("старше {} календарных месяцев", n.get()),
        Retention::Years(n) => format!("старше {} лет", n.get()),
        Retention::Before(date) => format!("до {date}, строго раньше"),
    }
}
