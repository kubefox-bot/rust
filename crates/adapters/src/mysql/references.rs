use super::MAX_BATCH_SIZE;
use hmail_cleaner_core::{
    cleanup::PortResult,
    maintenance::{FileReferences, MAX_REFERENCES},
    paths::message_guid,
};
use sqlx::{MySqlConnection, Row};

pub(super) async fn load(
    connection: &mut MySqlConnection,
    limit: usize,
) -> PortResult<FileReferences> {
    if limit == 0 || limit > MAX_REFERENCES {
        return Err("invalid file reference limit".into());
    }
    let upper: Option<i64> = sqlx::query_scalar("SELECT MAX(messageid) FROM hm_messages")
        .fetch_one(&mut *connection)
        .await?;
    let upper = upper.unwrap_or(0);
    let mut cursor = 0;
    let mut references = FileReferences::new();
    loop {
        let rows=sqlx::query("SELECT messageid,messagefilename FROM hm_messages FORCE INDEX(PRIMARY) WHERE messageid>? AND messageid<=? ORDER BY messageid LIMIT ?")
            .bind(cursor).bind(upper).bind(MAX_BATCH_SIZE as u32).fetch_all(&mut *connection).await?;
        if rows.is_empty() {
            return Ok(references);
        }
        for row in rows {
            let id: i64 = row.try_get("messageid")?;
            if id <= cursor || id > upper {
                return Err("invalid file reference cursor".into());
            }
            cursor = id;
            let filename: String = row.try_get("messagefilename")?;
            let guid = message_guid(&filename)?;
            if !references.contains(&guid) && references.len() == limit {
                return Err("file reference limit exceeded; orphan deletion blocked".into());
            }
            references.insert(guid);
        }
    }
}
