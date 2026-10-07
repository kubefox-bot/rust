use crate::commands::Report;
use chrono::NaiveDateTime;
use hmail_cleaner_core::cleanup::Progress;
use serde::Serialize;
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum Event<'a> {
    Server {
        mysql_version: &'a str,
        cutoff: NaiveDateTime,
    },
    Progress(&'a Progress),
    Result(&'a Report),
}
impl Event<'_> {
    pub(super) fn line(&self) -> std::io::Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec(self).map_err(std::io::Error::other)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}
