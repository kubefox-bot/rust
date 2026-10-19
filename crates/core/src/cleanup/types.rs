use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::{num::NonZeroU32, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: i64,
    pub account_id: i32,
    pub folder_id: i32,
    pub storage: MessageStorage,
    pub filename: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStorage {
    Queue,
    Account { address: String },
    Public,
}

#[derive(Debug, Clone, Copy)]
pub struct Selection {
    pub cutoff: NaiveDateTime,
    pub upper_id: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct Snapshot {
    pub upper_id: i64,
}

#[derive(Debug)]
pub struct Batch {
    pub last_id: i64,
    pub candidates: Vec<Candidate>,
    pub rejected: Vec<RowFailure>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RowFailure {
    pub id: i64,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct FileTarget {
    pub id: i64,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMetadata {
    pub created_at: std::time::SystemTime,
    pub modified_at: std::time::SystemTime,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct FileInspection {
    pub id: i64,
    pub path: PathBuf,
    pub result: Result<Option<FileMetadata>, FileFailure>,
}

#[derive(Debug)]
pub struct FileOutcome {
    pub id: i64,
    pub path: PathBuf,
    pub result: DeletionResult,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DeletionResult {
    Removed { size_bytes: u64 },
    AlreadyAbsent,
    Failed(FileFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileFailureKind {
    AccessDenied,
    InvalidPath,
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileFailure {
    pub kind: FileFailureKind,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct PruneOutcome {
    pub removed: u64,
    pub failures: Vec<DirectoryFailure>,
}

#[derive(Debug, Serialize)]
pub struct DirectoryFailure {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteResult {
    Committed(DeletedRows),
    CommitUncertain { reason: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeletedRows {
    pub messages: u64,
    pub metadata: u64,
    pub recipients: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub selected_bytes: u64,
    pub selected_files: u64,
    pub total_scan_files: Option<u64>,
    pub verified_files: u64,
    pub remaining_old_files: Option<u64>,
    pub examined_messages: u64,
    pub selected: u64,
    pub removed_files: u64,
    pub absent_files: u64,
    pub deleted_messages: u64,
    pub removed_directories: u64,
    pub failed: u64,
    pub removed_bytes: u64,
    pub deleted_metadata: u64,
    pub deleted_recipients: u64,
    pub orphan_metadata: u64,
    pub orphan_recipients: u64,
    pub selected_orphan_metadata: u64,
    pub selected_orphan_recipients: u64,
    pub scanned_files: u64,
    pub selected_orphan_files: u64,
    pub removed_orphan_files: u64,
}

#[derive(Debug)]
pub struct RunOptions {
    pub cutoff: NaiveDateTime,
    pub file_cutoff: std::time::SystemTime,
    pub batch_size: NonZeroU32,
    pub workers: u8,
    pub data_directory: PathBuf,
}
