use serde::Serialize;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrphanTable {
    Metadata,
    Recipients,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrphanRow {
    pub id: i64,
    pub parent_message_id: i64,
}
#[derive(Debug, Clone, Copy)]
pub struct OrphanSelection {
    pub table: OrphanTable,
    pub upper_id: i64,
}
#[derive(Debug)]
pub struct OrphanBatch {
    pub last_id: i64,
    pub rows: Vec<OrphanRow>,
}
pub type FileReferences = HashSet<Uuid>;
