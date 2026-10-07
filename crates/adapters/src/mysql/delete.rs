use super::{RepositoryError, DELIVERED_STATE, MAX_BATCH_SIZE};
use hmail_cleaner_core::cleanup::{Candidate, DeleteResult, PortResult};
use sqlx::{Connection, MySql, MySqlConnection, QueryBuilder, Row, Transaction};
use std::collections::{HashMap, HashSet};

const RELATED: &[(&str, &str)] = &[
    ("hm_message_metadata", "metadata_messageid"),
    ("hm_messagerecipients", "recipientmessageid"),
    ("hm_messages", "messageid"),
];

pub(super) async fn batch(
    connection: &mut MySqlConnection,
    candidates: &[Candidate],
) -> PortResult<DeleteResult> {
    if candidates.is_empty() {
        return Ok(DeleteResult::Committed);
    }
    if candidates.len() > MAX_BATCH_SIZE {
        return Err(RepositoryError::InvalidBatch("too many IDs").into());
    }
    let unique: HashSet<_> = candidates.iter().map(|message| message.id).collect();
    if unique.len() != candidates.len() {
        return Err(RepositoryError::InvalidBatch("duplicate IDs").into());
    }
    let mut transaction = connection.begin().await?;
    if let Err(original) = execute(&mut transaction, candidates).await {
        if let Err(rollback) = transaction.rollback().await {
            return Err(RepositoryError::Rollback {
                original: original.to_string(),
                rollback,
            }
            .into());
        }
        return Err(original);
    }
    match transaction.commit().await {
        Ok(()) => Ok(DeleteResult::Committed),
        Err(error) => Ok(DeleteResult::CommitUncertain {
            reason: error.to_string(),
        }),
    }
}

async fn execute(
    transaction: &mut Transaction<'_, MySql>,
    candidates: &[Candidate],
) -> PortResult<()> {
    let expected: HashMap<_, _> = candidates
        .iter()
        .map(|message| (message.id, message))
        .collect();
    let mut query = QueryBuilder::<MySql>::new(
        "SELECT m.messageid,m.messageaccountid,m.messagefolderid,m.messagefilename,
         m.messagecreatetime,m.messagetype,a.accountaddress
         FROM hm_messages m LEFT JOIN hm_accounts a ON a.accountid=m.messageaccountid
         WHERE m.messageid IN (",
    );
    bind_ids(&mut query, candidates.iter().map(|message| message.id));
    query.push(") FOR UPDATE");
    let rows = query.build().fetch_all(&mut **transaction).await?;
    if rows.len() != candidates.len() {
        return Err(RepositoryError::StaleBatch.into());
    }
    for row in rows {
        let id: i64 = row.try_get("messageid")?;
        let Some(message) = expected.get(&id) else {
            return Err(RepositoryError::StaleBatch.into());
        };
        let current = Candidate {
            id,
            account_id: row.try_get("messageaccountid")?,
            folder_id: row.try_get("messagefolderid")?,
            filename: row.try_get("messagefilename")?,
            created_at: row.try_get("messagecreatetime")?,
            account_address: row
                .try_get::<Option<String>, _>("accountaddress")?
                .ok_or(RepositoryError::StaleBatch)?,
        };
        if &current != *message || row.try_get::<i8, _>("messagetype")? != DELIVERED_STATE {
            return Err(RepositoryError::StaleBatch.into());
        }
    }
    for (table, column) in RELATED {
        let mut deletion =
            QueryBuilder::<MySql>::new(format!("DELETE FROM {table} WHERE {column} IN ("));
        bind_ids(&mut deletion, candidates.iter().map(|message| message.id));
        deletion.push(")");
        let result = deletion.build().execute(&mut **transaction).await?;
        if *table == "hm_messages" && result.rows_affected() != candidates.len() as u64 {
            return Err(RepositoryError::StaleBatch.into());
        }
    }
    Ok(())
}

pub(super) async fn verify(connection: &mut MySqlConnection, ids: &[i64]) -> PortResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    for (table, column) in RELATED {
        let mut query =
            QueryBuilder::<MySql>::new(format!("SELECT COUNT(*) FROM {table} WHERE {column} IN ("));
        bind_ids(&mut query, ids.iter().copied());
        query.push(")");
        let count: i64 = query
            .build_query_scalar()
            .fetch_one(&mut *connection)
            .await?;
        if count != 0 {
            return Err(RepositoryError::Verification.into());
        }
    }
    Ok(())
}

fn bind_ids<'a>(query: &mut QueryBuilder<'a, MySql>, ids: impl Iterator<Item = i64>) {
    let mut separated = query.separated(",");
    for id in ids {
        separated.push_bind(id);
    }
}
