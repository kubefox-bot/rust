use hmail_cleaner_core::cleanup::{Stage, Summary};
use hmail_cleaner_protocol::{JobStage, SummaryDto};

pub(super) fn stage(value: Stage) -> JobStage {
    match value {
        Stage::Preflight => JobStage::Preflight,
        Stage::Snapshot => JobStage::Snapshot,
        Stage::Selection => JobStage::Selection,
        Stage::Files => JobStage::Files,
        Stage::Database => JobStage::Database,
        Stage::Verification => JobStage::Verification,
        Stage::Directories => JobStage::Directories,
        Stage::Complete => JobStage::Complete,
        Stage::Cancelled => JobStage::Cancelled,
        Stage::OrphanDatabase => JobStage::OrphanDatabase,
        Stage::References => JobStage::References,
        Stage::Scan => JobStage::Scan,
    }
}
pub(super) fn summary(value: &Summary) -> SummaryDto {
    SummaryDto {
        selected_bytes: value.selected_bytes.into(),
        selected_files: value.selected_files.into(),
        total_scan_files: value.total_scan_files.map(Into::into),
        verified_files: value.verified_files.into(),
        remaining_old_files: value.remaining_old_files.map(Into::into),
        examined_messages: value.examined_messages.into(),
        selected: value.selected.into(),
        removed_files: value.removed_files.into(),
        absent_files: value.absent_files.into(),
        deleted_messages: value.deleted_messages.into(),
        removed_directories: value.removed_directories.into(),
        failed: value.failed.into(),
        removed_bytes: value.removed_bytes.into(),
        deleted_metadata: value.deleted_metadata.into(),
        deleted_recipients: value.deleted_recipients.into(),
        orphan_metadata: value.orphan_metadata.into(),
        orphan_recipients: value.orphan_recipients.into(),
        selected_orphan_metadata: value.selected_orphan_metadata.into(),
        selected_orphan_recipients: value.selected_orphan_recipients.into(),
        scanned_files: value.scanned_files.into(),
        selected_orphan_files: value.selected_orphan_files.into(),
        removed_orphan_files: value.removed_orphan_files.into(),
    }
}
