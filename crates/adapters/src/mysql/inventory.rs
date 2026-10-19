mod index;
use super::{registry::Registry, RepositoryError};
use hmail_cleaner_core::{
    cleanup::{Cancellation, Candidate, MessageStorage, PortResult},
    maintenance::StoredMessage,
    paths::resolve_path,
};
pub(super) use index::Index;
use sqlx::{MySqlConnection, Row};
use std::path::Path;
const READ_BATCH_SIZE: u32 = hmail_cleaner_core::cleanup::DEFAULT_BATCH_SIZE;
pub(super) async fn load(
    connection: &mut MySqlConnection,
    registry: &Registry,
    root: &Path,
    upper: i64,
    cancel: &Cancellation,
) -> PortResult<Index> {
    let mut index = Index::create().await?;
    let mut cursor = 0;
    loop {
        cancel.check()?;
        let rows = sqlx::query("SELECT messageid,messageaccountid,messagefolderid,messagefilename,messagecreatetime,messagetype FROM hm_messages FORCE INDEX(PRIMARY) WHERE messageid>? AND messageid<=? ORDER BY messageid LIMIT ?")
            .bind(cursor).bind(upper).bind(READ_BATCH_SIZE).fetch_all(&mut *connection).await?;
        cancel.check()?;
        if rows.is_empty() {
            return Ok(index);
        }
        let mut batch = Vec::with_capacity(rows.len());
        for row in rows {
            let id: i64 = row.try_get("messageid")?;
            if id <= cursor || id > upper {
                return Err("Некорректный курсор карты БД".into());
            }
            cursor = id;
            let account_id = row.try_get("messageaccountid")?;
            let folder_id = row.try_get("messagefolderid")?;
            let message_type = row.try_get("messagetype")?;
            let storage = if message_type == super::DELIVERED_STATE {
                super::storage::resolve(
                    account_id,
                    folder_id,
                    registry.accounts.get(&account_id).map(String::as_str),
                    registry.folders.get(&folder_id).copied(),
                )?
            } else if super::QUEUE_STATES.contains(&message_type) {
                MessageStorage::Queue
            } else {
                return Err(format!(
                    "Неподдерживаемое состояние письма {id}: {message_type}; очистка не начата"
                )
                .into());
            };
            let candidate = Candidate {
                id,
                account_id,
                folder_id,
                storage,
                filename: row.try_get("messagefilename")?,
                created_at: row.try_get("messagecreatetime")?,
            };
            let path = resolve_path(root, &candidate).map_err(|error| {
                RepositoryError::InvalidStoredPath {
                    id,
                    reason: error.to_string(),
                }
            })?;
            batch.push(StoredMessage {
                candidate,
                message_type,
                path,
            });
        }
        index.insert(&batch).await?;
    }
}
