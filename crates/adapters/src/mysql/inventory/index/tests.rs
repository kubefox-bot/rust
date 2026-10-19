use super::*;
use hmail_cleaner_core::cleanup::{Candidate, MessageStorage};

fn row(id: i64, path: &str) -> StoredMessage {
    StoredMessage {
        candidate: Candidate {
            id,
            account_id: 0,
            folder_id: 0,
            storage: MessageStorage::Queue,
            filename: path.into(),
            created_at: chrono::NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        },
        message_type: 1,
        path: path.into(),
    }
}

#[tokio::test]
async fn duplicates_across_import_batches_are_counted_without_retaining_all_rows() {
    let mut index = Index::create().await.unwrap();
    let first = row(1, "/Data/Mail/a.eml");
    index.insert(std::slice::from_ref(&first)).await.unwrap();
    for id in 2..=100 {
        index
            .insert(&[row(id, "\\data\\mail\\A.EML")])
            .await
            .unwrap();
    }
    let other = row(101, "/Data/Other/a.eml");
    index.insert(std::slice::from_ref(&other)).await.unwrap();
    let groups = index
        .match_files(&["/DATA/mail/a.eml".into()])
        .await
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].references, 100);
    assert_eq!(groups[0].row, first);
    let unseen = index.unmatched_batch(0, 1).await.unwrap();
    assert_eq!(unseen.len(), 1);
    assert_eq!(unseen[0].row, other);
    assert_eq!(unseen[0].references, 1);
    assert!(index.unmatched_batch(101, 1).await.unwrap().is_empty());
    index.close().await.unwrap();
}

#[tokio::test]
async fn unmatched_cursor_skips_seen_paths_and_advances_over_id_gaps() {
    let mut index = Index::create().await.unwrap();
    index
        .insert(&[
            row(2, "/Data/2.eml"),
            row(99, "/Data/99.eml"),
            row(1000, "/Data/1000.eml"),
        ])
        .await
        .unwrap();
    assert!(index.match_files(&[]).await.unwrap().is_empty());
    assert!(index
        .match_files(&["/Data/orphan.eml".into()])
        .await
        .unwrap()
        .is_empty());
    index.match_files(&["/Data/99.eml".into()]).await.unwrap();
    let first = index.unmatched_batch(0, 1).await.unwrap();
    assert_eq!(first[0].row.candidate.id, 2);
    let second = index.unmatched_batch(2, 1).await.unwrap();
    assert_eq!(second[0].row.candidate.id, 1000);
    assert!(index.unmatched_batch(1000, 1).await.unwrap().is_empty());
    index.close().await.unwrap();
    let mut next = Index::create().await.unwrap();
    assert!(next.unmatched_batch(0, 1).await.unwrap().is_empty());
    next.close().await.unwrap();
}

#[tokio::test]
async fn exhausted_scratch_storage_returns_an_error() {
    let mut index = Index::create().await.unwrap();
    sqlx::query("PRAGMA max_page_count=4")
        .execute(&mut index.connection)
        .await
        .unwrap();
    let rows = (1..=100)
        .map(|id| row(id, &format!("/Data/{id}.eml")))
        .collect::<Vec<_>>();
    let error = index.insert(&rows).await.unwrap_err();
    assert!(
        error.to_string().contains("database or disk is full"),
        "{error}"
    );
    index.close().await.unwrap();
}
