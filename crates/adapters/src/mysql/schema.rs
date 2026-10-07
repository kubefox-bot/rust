use super::RepositoryError;
use hmail_cleaner_core::cleanup::PortResult;
use sqlx::MySqlConnection;
use std::collections::HashMap;

const TABLES: &[&str] = &[
    "hm_messages",
    "hm_accounts",
    "hm_imapfolders",
    "hm_message_metadata",
    "hm_messagerecipients",
];

const COLUMNS: &[(&str, &str, &str)] = &[
    ("hm_messages", "messageid", "bigint"),
    ("hm_messages", "messageaccountid", "int"),
    ("hm_messages", "messagefolderid", "int"),
    ("hm_messages", "messagefilename", "varchar"),
    ("hm_messages", "messagetype", "tinyint"),
    ("hm_messages", "messagecreatetime", "datetime"),
    ("hm_accounts", "accountid", "int"),
    ("hm_accounts", "accountaddress", "varchar"),
    ("hm_imapfolders", "folderid", "int"),
    ("hm_imapfolders", "folderaccountid", "int"),
    ("hm_message_metadata", "metadata_messageid", "bigint"),
    ("hm_messagerecipients", "recipientmessageid", "bigint"),
];

pub(super) async fn verify(connection: &mut MySqlConnection) -> PortResult<()> {
    let tables: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT TABLE_NAME,ENGINE FROM INFORMATION_SCHEMA.TABLES
         WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME IN
         ('hm_messages','hm_accounts','hm_imapfolders','hm_message_metadata','hm_messagerecipients')"
    ).fetch_all(&mut *connection).await?;
    let engines: HashMap<_, _> = tables.into_iter().collect();
    for table in TABLES {
        if !engines
            .get(*table)
            .and_then(Option::as_deref)
            .is_some_and(|engine| engine.eq_ignore_ascii_case("InnoDB"))
        {
            return Err(RepositoryError::Schema(format!("{table} must be an InnoDB table")).into());
        }
    }
    let columns: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT TABLE_NAME,COLUMN_NAME,DATA_TYPE FROM INFORMATION_SCHEMA.COLUMNS
         WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME IN
         ('hm_messages','hm_accounts','hm_imapfolders','hm_message_metadata','hm_messagerecipients')"
    ).fetch_all(&mut *connection).await?;
    let types: HashMap<_, _> = columns
        .into_iter()
        .map(|(table, column, kind)| ((table, column), kind))
        .collect();
    for (table, column, kind) in COLUMNS {
        if !types
            .get(&(table.to_string(), column.to_string()))
            .is_some_and(|actual| actual == kind)
        {
            return Err(RepositoryError::Schema(format!("{table}.{column} must be {kind}")).into());
        }
    }
    let primary: Vec<(String,String)> = sqlx::query_as(
        "SELECT TABLE_NAME,COLUMN_NAME FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE WHERE TABLE_SCHEMA=DATABASE()
         AND CONSTRAINT_NAME='PRIMARY' AND TABLE_NAME IN ('hm_message_metadata','hm_messagerecipients')"
    ).fetch_all(&mut *connection).await?;
    for kind in [
        hmail_cleaner_core::maintenance::OrphanTable::Metadata,
        hmail_cleaner_core::maintenance::OrphanTable::Recipients,
    ] {
        let table = super::orphans::descriptor(kind);
        let keys = primary
            .iter()
            .filter(|(name, _)| name == table.name)
            .map(|(_, column)| column.as_str())
            .collect::<Vec<_>>();
        if keys != [table.id]
            || types
                .get(&(table.name.to_string(), table.id.to_string()))
                .is_none_or(|kind| kind != "bigint")
        {
            return Err(RepositoryError::Schema(format!(
                "{}.{} must be the sole bigint primary key",
                table.name, table.id
            ))
            .into());
        }
    }
    Ok(())
}
