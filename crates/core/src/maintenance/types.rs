use serde::Serialize;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    Fast,
    Full,
}
#[derive(Debug, Clone, Copy)]
pub enum ScanOptions {
    Fast,
    Full,
}
impl ScanOptions {
    pub fn requires_stopped(self, operation: super::workflow::Operation) -> bool {
        use super::workflow::Operation;
        match (self, operation) {
            (Self::Fast, Operation::Preview) => false,
            (Self::Fast | Self::Full, Operation::Clean) | (Self::Full, Operation::Preview) => true,
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileObservation {
    pub path: std::path::PathBuf,
    pub guid: Uuid,
    pub size_bytes: u64,
    pub modified_at: std::time::SystemTime,
    pub created_at: std::time::SystemTime,
}
#[derive(Debug, Default)]
pub struct ScanStats {
    pub examined_files: u64,
}
#[derive(Debug)]
pub struct ScanBatch {
    pub completed_directories: Vec<std::path::PathBuf>,
    pub visited_entries: u64,
    pub examined_files: u64,
    pub files: Vec<FileObservation>,
    pub finished: bool,
}
#[derive(Debug)]
pub struct OrphanFileOutcome {
    pub observation: FileObservation,
    pub result: crate::cleanup::DeletionResult,
}

/// A database row and its resolved physical location, including queue entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMessage {
    pub candidate: crate::cleanup::Candidate,
    pub message_type: i8,
    pub path: std::path::PathBuf,
}
