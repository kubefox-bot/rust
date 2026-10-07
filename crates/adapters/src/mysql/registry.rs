use hmail_cleaner_core::cleanup::PortResult;
use sqlx::{MySql, Transaction};
use std::collections::HashMap;

pub(super) struct Registry {
    pub accounts: HashMap<i32, String>,
    pub folders: HashMap<i32, u32>,
}

pub(super) async fn load(transaction: &mut Transaction<'_, MySql>) -> PortResult<Registry> {
    let accounts: Vec<(i32, String)> =
        sqlx::query_as("SELECT accountid,accountaddress FROM hm_accounts")
            .fetch_all(&mut **transaction)
            .await?;
    let folders: Vec<(i32, u32)> =
        sqlx::query_as("SELECT folderid,folderaccountid FROM hm_imapfolders")
            .fetch_all(&mut **transaction)
            .await?;
    Ok(Registry {
        accounts: accounts.into_iter().collect(),
        folders: folders.into_iter().collect(),
    })
}
