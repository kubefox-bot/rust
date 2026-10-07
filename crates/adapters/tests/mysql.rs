mod fixtures;

use fixtures::{date, name, Fixture};
use hmail_cleaner_adapters::mysql::{DatabaseConfig, MySqlRepository};
use hmail_cleaner_core::cleanup::{DeleteResult, Repository, Selection};
use sqlx::Executor;

const OLD: &str = "2026-01-01 10:00:00";
const BOUNDARY: &str = "2026-09-07 12:00:00";

fn selection(upper_id: i64) -> Selection {
    Selection {
        cutoff: date(BOUNDARY),
        upper_id,
    }
}

#[test]
fn config_rejects_missing_database() {
    assert!(DatabaseConfig::parse("mysql://root:dummy@127.0.0.1").is_err());
}

#[test]
fn config_debug_hides_credentials() {
    let config = DatabaseConfig::parse("mysql://root:do-not-print-this@127.0.0.1/hmaildb").unwrap();
    assert!(!format!("{config:?}").contains("do-not-print-this"));
}

#[tokio::test]
#[ignore = "requires mise run test:db disposable MySQL 5.7.21"]
async fn selects_only_old_delivered_accounts_across_folders() {
    let mut fixture = Fixture::create().await;
    for (id, account, folder, state, created) in [
        (1, 1, 10, 2, OLD),
        (2, 1, 20, 2, OLD),
        (3, 2, 30, 2, OLD),
        (4, 0, 0, 1, OLD),
        (5, 0, 40, 2, OLD),
        (6, 1, 10, 2, "2026-10-01 12:00:00"),
        (7, 1, 20, 2, BOUNDARY),
    ] {
        fixture
            .add(id, account, folder, state, created, &name(id))
            .await;
    }
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    assert_eq!(
        batch
            .candidates
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(batch.candidates[0].account_address, "info@chem.orp");
    assert_eq!(batch.candidates[2].account_address, "sales@chem.orp");
    assert!(batch.rejected.is_empty());
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn keyset_cursor_advances_over_rejected_rows() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    fixture.add(2, 9999, 10, 2, OLD, &name(2)).await;
    fixture.add(3, 1, 10, 2, OLD, &name(3)).await;
    fixture.add(4, 1, 30, 2, OLD, &name(4)).await;
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let first = repository
        .next_batch(selection(snapshot.upper_id), 0, 1)
        .await
        .unwrap();
    assert_eq!(first.last_id, 1);
    let rejected = repository
        .next_batch(selection(snapshot.upper_id), first.last_id, 1)
        .await
        .unwrap();
    assert_eq!(rejected.last_id, 2);
    assert_eq!(rejected.rejected[0].id, 2);
    assert!(rejected.candidates.is_empty());
    let next = repository
        .next_batch(selection(snapshot.upper_id), rejected.last_id, 1)
        .await
        .unwrap();
    assert_eq!(next.candidates[0].id, 3);
    let mismatch = repository
        .next_batch(selection(snapshot.upper_id), next.last_id, 1)
        .await
        .unwrap();
    assert_eq!(mismatch.rejected[0].id, 4);
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn legacy_and_bare_duplicate_names_are_rejected() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    fixture
        .add(
            2,
            1,
            10,
            2,
            "2026-10-01 10:00:00",
            &format!("D:\\Data\\chem.orp\\info\\00\\{}", name(1).to_lowercase()),
        )
        .await;
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    assert!(batch.candidates.is_empty());
    assert_eq!(batch.rejected[0].id, 1);
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn deletes_only_requested_ids_and_their_dependents() {
    let mut fixture = Fixture::create().await;
    for id in [1, 2, 3] {
        fixture.add(id, 1, 10, 2, OLD, &name(id)).await;
        fixture.add_dependents(id).await;
    }
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    let allowed = vec![batch.candidates[0].clone(), batch.candidates[2].clone()];
    assert_eq!(
        repository.delete_batch(&allowed).await.unwrap(),
        DeleteResult::Committed
    );
    repository.verify_deleted(&[1, 3]).await.unwrap();
    for (table, column) in [
        ("hm_messages", "messageid"),
        ("hm_message_metadata", "metadata_messageid"),
        ("hm_messagerecipients", "recipientmessageid"),
    ] {
        assert_eq!(fixture.ids(table, column).await, vec![2]);
    }
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn empty_delete_is_a_noop() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    let mut repository = fixture.repository().await;
    repository.snapshot().await.unwrap();
    assert_eq!(
        repository.delete_batch(&[]).await.unwrap(),
        DeleteResult::Committed
    );
    assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![1]);
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn failed_child_delete_rolls_back_parent() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    fixture.add_dependents(1).await;
    fixture
        .admin
        .execute(
            "CREATE TRIGGER fail_recipient_delete BEFORE DELETE ON hm_messagerecipients
         FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='fixture failure'",
        )
        .await
        .unwrap();
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    assert!(repository.delete_batch(&batch.candidates).await.is_err());
    for (table, column) in [
        ("hm_messages", "messageid"),
        ("hm_message_metadata", "metadata_messageid"),
        ("hm_messagerecipients", "recipientmessageid"),
    ] {
        assert_eq!(fixture.ids(table, column).await, vec![1]);
    }
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn second_connection_cannot_acquire_cleanup_lock() {
    let fixture = Fixture::create().await;
    let first = fixture.repository().await;
    assert!(MySqlRepository::connect(&fixture.config).await.is_err());
    first.close().await.unwrap();
    let next = fixture.repository().await;
    next.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn changed_row_is_not_deleted() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    fixture
        .admin
        .execute("UPDATE hm_messages SET messagefolderid=20 WHERE messageid=1")
        .await
        .unwrap();
    assert!(repository.delete_batch(&batch.candidates).await.is_err());
    assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![1]);
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn unsupported_storage_engine_is_rejected_before_selection() {
    let mut fixture = Fixture::create().await;
    fixture
        .admin
        .execute("ALTER TABLE hm_message_metadata ENGINE=MyISAM")
        .await
        .unwrap();
    assert!(MySqlRepository::connect(&fixture.config).await.is_err());
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn snapshot_excludes_later_insertions_and_query_uses_primary_range() {
    use sqlx::{Column, Row};
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    fixture.add(2, 1, 10, 2, OLD, &name(2)).await;
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    assert_eq!(
        batch
            .candidates
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        vec![1]
    );
    let explain = sqlx::query("EXPLAIN SELECT messageid,messageaccountid,messagefolderid,messagefilename,messagecreatetime FROM hm_messages FORCE INDEX(PRIMARY) WHERE messageid>0 AND messageid<=2 AND messagetype=2 AND messageaccountid>0 AND messagecreatetime<'2026-09-07 12:00:00' ORDER BY messageid LIMIT 1000").fetch_one(&mut fixture.admin).await.unwrap();
    // MySQL 5.7 EXPLAIN metadata does not populate SQLx's name lookup.
    let value = |name: &str| {
        let index = explain
            .columns()
            .iter()
            .position(|column| column.name() == name)
            .unwrap();
        explain.get::<String, _>(index)
    };
    assert_eq!(value("key"), "PRIMARY");
    assert_eq!(value("type"), "range");
    assert!(!value("Extra").contains("filesort"));
    repository.close().await.unwrap();
    fixture.finish().await;
}
