use chrono::NaiveDateTime;
use serde::Serialize;
use std::{num::NonZeroU32, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: i64,
    pub account_id: i32,
    pub folder_id: i32,
    pub account_address: String,
    pub filename: String,
    pub created_at: NaiveDateTime,
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

#[derive(Debug)]
pub struct FileTarget {
    pub id: i64,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct FileOutcome {
    pub id: i64,
    pub path: PathBuf,
    pub result: DeletionResult,
}

#[derive(Debug)]
pub enum DeletionResult {
    Removed,
    AlreadyAbsent,
    Failed(FileFailure),
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileFailureKind {
    AccessDenied,
    InvalidPath,
    Io,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteResult {
    Committed,
    CommitUncertain,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub selected: u64,
    pub removed_files: u64,
    pub absent_files: u64,
    pub deleted_messages: u64,
    pub removed_directories: u64,
    pub failed: u64,
}

#[derive(Debug)]
pub struct RunOptions {
    pub cutoff: NaiveDateTime,
    pub batch_size: NonZeroU32,
    pub workers: u8,
    pub data_directory: PathBuf,
}
