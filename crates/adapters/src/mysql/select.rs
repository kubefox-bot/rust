use super::{duplicates, registry::Registry, RepositoryError, DELIVERED_STATE, MAX_BATCH_SIZE};
use chrono::{Datelike, NaiveDateTime};
use hmail_cleaner_core::cleanup::{Batch, Candidate, PortResult, RowFailure, Selection};
use sqlx::{mysql::MySqlRow, MySqlConnection, Row};
use std::collections::HashSet;

const MIN_MYSQL_YEAR: i32 = 1000;
const MAX_MYSQL_YEAR: i32 = 9999;
const QUERY: &str =
    "SELECT messageid,messageaccountid,messagefolderid,messagefilename,messagecreatetime
     FROM hm_messages FORCE INDEX (PRIMARY)
     WHERE messageid>? AND messageid<=? AND messagetype=?
       AND messageaccountid>0 AND messagecreatetime<?
     ORDER BY messageid LIMIT ?";

pub(super) async fn next(
    connection: &mut MySqlConnection,
    registry: &Registry,
    duplicate_names: &HashSet<String>,
    selection: Selection,
    cursor: i64,
    limit: u32,
) -> PortResult<Batch> {
    if limit == 0 || limit as usize > MAX_BATCH_SIZE {
        return Err(RepositoryError::InvalidBatch("limit must be between 1 and 10000").into());
    }
    if !(MIN_MYSQL_YEAR..=MAX_MYSQL_YEAR).contains(&selection.cutoff.year()) {
        return Err(RepositoryError::InvalidBatch("cutoff is outside MySQL DATETIME range").into());
    }
    let rows = sqlx::query(QUERY)
        .bind(cursor)
        .bind(selection.upper_id)
        .bind(DELIVERED_STATE)
        .bind(selection.cutoff)
        .bind(limit)
        .fetch_all(connection)
        .await?;
    let last_id = match rows.last() {
        Some(row) => row.try_get("messageid")?,
        None => cursor,
    };
    let mut batch = Batch {
        last_id,
        candidates: Vec::new(),
        rejected: Vec::new(),
    };
    for row in rows {
        let id: i64 = row.try_get("messageid")?;
        match candidate(&row, registry, duplicate_names)? {
            Ok(message) => batch.candidates.push(message),
            Err(reason) => batch.rejected.push(RowFailure { id, reason }),
        }
    }
    Ok(batch)
}

fn candidate(
    row: &MySqlRow,
    registry: &Registry,
    duplicate_names: &HashSet<String>,
) -> PortResult<Result<Candidate, String>> {
    let id = row.try_get("messageid")?;
    let account_id: i32 = row.try_get("messageaccountid")?;
    let folder_id: i32 = row.try_get("messagefolderid")?;
    let filename: String = row.try_get("messagefilename")?;
    let created_at: NaiveDateTime = row.try_get("messagecreatetime")?;
    let Some(address) = registry.accounts.get(&account_id) else {
        return Ok(Err("account is absent".into()));
    };
    let owner = registry.folders.get(&folder_id);
    if owner.copied() != u32::try_from(account_id).ok() {
        return Ok(Err(
            "IMAP folder is absent or belongs to another account".into()
        ));
    }
    if duplicate_names.contains(&duplicates::basename(&filename)) {
        return Ok(Err("filename has multiple database references".into()));
    }
    Ok(Ok(Candidate {
        id,
        account_id,
        folder_id,
        account_address: address.clone(),
        filename,
        created_at,
    }))
}
