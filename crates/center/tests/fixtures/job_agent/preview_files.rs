use super::Control;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use hmail_cleaner_protocol::{
    JobId, Operation, PreviewFile, PreviewFileSort, PreviewFilesPage, PreviewFilesQuery,
    SortDirection,
};
use std::sync::atomic::Ordering;

pub(super) async fn read(
    State(control): State<Control>,
    Path(id): Path<JobId>,
    Query(query): Query<PreviewFilesQuery>,
) -> Result<Json<PreviewFilesPage>, StatusCode> {
    query.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
    let snapshot = control.receipt(id).ok_or(StatusCode::NOT_FOUND)?;
    let job = snapshot.job.as_ref().ok_or(StatusCode::CONFLICT)?;
    if job.operation != Operation::Preview {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Synthetic metadata only: no filesystem or real mailbox is read by the fixture.
    let mut rows = (0..snapshot.summary.selected_files.get().min(12))
        .map(|index| PreviewFile {
            id: format!("fixture-{index:04}"),
            path: format!(
                r"C:\hmail-mvp-synthetic\mail\account\message-{:04}.eml",
                index + 1
            ),
            size_bytes: ((index % 3 + 1) * 2 * 1024 * 1024).into(),
            created_at: job.resolved_cutoff.file_cutoff_utc
                - chrono::Duration::days(index as i64 + 1),
            orphan: index % 4 == 0,
        })
        .filter(|row| {
            query.created_from.is_none_or(|from| row.created_at >= from)
                && query.created_to.is_none_or(|to| row.created_at <= to)
                && query.orphan.is_none_or(|orphan| row.orphan == orphan)
        })
        .filter(|row| {
            row.path
                .to_lowercase()
                .contains(&query.search.to_lowercase())
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        let order = match query.sort {
            PreviewFileSort::Path => left.path.cmp(&right.path),
            PreviewFileSort::Size => left.size_bytes.cmp(&right.size_bytes),
            PreviewFileSort::CreatedAt => left.created_at.cmp(&right.created_at),
        }
        .then_with(|| left.id.cmp(&right.id));
        match query.direction {
            SortDirection::Asc => order,
            SortDirection::Desc => order.reverse(),
        }
    });
    let total = rows.len() as u64;
    let page = rows
        .into_iter()
        .skip(query.offset as usize)
        .take(usize::from(query.limit))
        .collect();
    Ok(Json(PreviewFilesPage {
        job_id: if control.0.wrong_file_job.load(Ordering::SeqCst) {
            JobId::new()
        } else {
            id
        },
        rows: page,
        total: total.into(),
        revision: snapshot.revision,
        complete: !snapshot.phase.is_active(),
    }))
}
