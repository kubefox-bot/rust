use hmail_cleaner_core::cleanup::PortResult;
use sqlx::{MySql, Transaction};
use std::collections::HashSet;

const QUERY: &str =
    "SELECT LOWER(SUBSTRING_INDEX(REPLACE(messagefilename,CHAR(92),'/'),'/',-1)) AS basename
     FROM hm_messages GROUP BY basename HAVING COUNT(*)>1";

pub(super) async fn load(transaction: &mut Transaction<'_, MySql>) -> PortResult<HashSet<String>> {
    let names: Vec<String> = sqlx::query_scalar(QUERY)
        .fetch_all(&mut **transaction)
        .await?;
    Ok(names.into_iter().collect())
}

pub(super) fn basename(filename: &str) -> String {
    filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(filename)
        .to_ascii_lowercase()
}
