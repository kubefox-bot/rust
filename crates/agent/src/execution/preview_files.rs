use crate::jobs::JobRepository;
use hmail_cleaner_core::{cleanup::PortResult, maintenance::recursive::SelectedFile};
use hmail_cleaner_protocol::{JobId, JobSnapshot, PreviewFile};
use sha2::{Digest, Sha256};
use tokio::sync::broadcast;

pub(super) async fn persist(
    id: JobId,
    files: &[SelectedFile],
    repository: &JobRepository,
    events: &broadcast::Sender<JobSnapshot>,
) -> PortResult<()> {
    let rows = files
        .iter()
        .map(|selected| {
            let path = selected
                .file
                .path
                .to_str()
                .ok_or("Preview file path is not valid UTF-8")?
                .to_owned();
            let normalized = path.replace('\\', "/").to_lowercase();
            let id = format!("{:x}", Sha256::digest(normalized.as_bytes()));
            let row = PreviewFile {
                id,
                path,
                size_bytes: selected.file.size_bytes.into(),
                created_at: selected.file.created_at.into(),
                orphan: selected.orphan,
            };
            row.validate()
                .map_err(|_| "Preview file metadata exceeds protocol bounds")?;
            Ok(row)
        })
        .collect::<PortResult<Vec<_>>>()?;
    let snapshot = repository
        .append_preview_files(id, &rows)
        .await
        .map_err(|_| "Preview file persistence failed")?;
    let _ = events.send(snapshot);
    Ok(())
}
