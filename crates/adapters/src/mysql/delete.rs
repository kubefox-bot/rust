use super::{ids::bind as bind_ids, transaction, RepositoryError, DELIVERED_STATE, MAX_BATCH_SIZE};
use hmail_cleaner_core::cleanup::{Candidate, DeleteResult, DeletedRows, PortResult};
use sqlx::{Connection, MySql, MySqlConnection, QueryBuilder, Row, Transaction};
use std::collections::{HashMap, HashSet};

const METADATA: (&str, &str) = ("hm_message_metadata", "metadata_messageid");
const RECIPIENTS: (&str, &str) = ("hm_messagerecipients", "recipientmessageid");
const MESSAGES: (&str, &str) = ("hm_messages", "messageid");
const RELATED: &[(&str, &str)] = &[METADATA, RECIPIENTS, MESSAGES];

pub(super) async fn verify_access(connection: &mut MySqlConnection) -> PortResult<()> {
    let mut transaction = connection.begin().await?;
    let result = probe(&mut transaction).await;
    let rollback = transaction.rollback().await;
    match (result, rollback) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(original), Ok(())) => Err(original),
        (Ok(()), Err(error)) => Err(error.into()),
        (Err(original), Err(rollback)) => Err(RepositoryError::Rollback {
            original: original.to_string(),
            rollback,
        }
        .into()),
    }
}
async fn probe(transaction: &mut Transaction<'_, MySql>) -> PortResult<()> {
    for (table, column) in RELATED {
        sqlx::query(&format!("SELECT {column} FROM {table} WHERE 1=0"))
            .execute(&mut **transaction)
            .await?;
        sqlx::query(&format!("DELETE FROM {table} WHERE 1=0"))
            .execute(&mut **transaction)
            .await?;
    }
    Ok(())
}

pub(super) async fn batch(
    connection: &mut MySqlConnection,
    candidates: &[Candidate],
) -> PortResult<DeleteResult> {
    if candidates.is_empty() {
        return Ok(DeleteResult::Committed(DeletedRows::default()));
    }
    if candidates.len() > MAX_BATCH_SIZE {
        return Err(RepositoryError::InvalidBatch("too many IDs").into());
    }
    let unique: HashSet<_> = candidates.iter().map(|message| message.id).collect();
    if unique.len() != candidates.len() {
        return Err(RepositoryError::InvalidBatch("duplicate IDs").into());
    }
    let mut transaction = connection.begin().await?;
    let rows = match execute(&mut transaction, candidates).await {
        Ok(rows) => rows,
        Err(original) => {
            return transaction::rollback(transaction, original).await;
        }
    };
    Ok(transaction::commit(transaction, rows).await)
}

async fn execute(
    transaction: &mut Transaction<'_, MySql>,
    candidates: &[Candidate],
) -> PortResult<DeletedRows> {
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
    let metadata = delete_ids(transaction, METADATA, candidates).await?;
    let recipients = delete_ids(transaction, RECIPIENTS, candidates).await?;
    let messages = delete_ids(transaction, MESSAGES, candidates).await?;
    if messages != candidates.len() as u64 {
        return Err(RepositoryError::StaleBatch.into());
    }
    Ok(DeletedRows {
        messages,
        metadata,
        recipients,
    })
}

async fn delete_ids(
    transaction: &mut Transaction<'_, MySql>,
    (table, column): (&str, &str),
    candidates: &[Candidate],
) -> PortResult<u64> {
    let mut deletion =
        QueryBuilder::<MySql>::new(format!("DELETE FROM {table} WHERE {column} IN ("));
    bind_ids(&mut deletion, candidates.iter().map(|message| message.id));
    deletion.push(")");
    Ok(deletion
        .build()
        .execute(&mut **transaction)
        .await?
        .rows_affected())
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
