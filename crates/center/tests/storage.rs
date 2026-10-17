use hmail_cleaner_center::storage::{CenterStore, DataDirectory};
#[tokio::test]
async fn directory_lock_and_corrupt_database_are_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    assert!(DataDirectory::acquire(root.path()).is_err());
    let bytes = b"this is not a SQLite database";
    std::fs::write(root.path().join("center.sqlite"), bytes).unwrap();
    assert!(CenterStore::open(&lease).await.is_err());
    assert_eq!(
        std::fs::read(root.path().join("center.sqlite")).unwrap(),
        bytes
    );
}
#[tokio::test]
async fn unknown_schema_is_preserved_and_the_store_owns_the_lock() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    let store = CenterStore::open(&lease).await.unwrap();
    drop(lease);
    assert!(DataDirectory::acquire(root.path()).is_err());
    drop(store);
    // Pool resources can finish closing asynchronously; use a separate directory for schema probe.
    let other = tempfile::tempdir().unwrap();
    let database = other.path().join("center.sqlite");
    use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection};
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&database)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("PRAGMA user_version=999")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    let before = std::fs::read(&database).unwrap();
    let lease = DataDirectory::acquire(other.path()).unwrap();
    assert!(CenterStore::open(&lease).await.is_err());
    assert_eq!(std::fs::read(database).unwrap(), before);
}
