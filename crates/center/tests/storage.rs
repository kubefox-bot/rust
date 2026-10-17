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
    let lease = DataDirectory::acquire(other.path()).unwrap();
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
    assert!(CenterStore::open(&lease).await.is_err());
    assert_eq!(std::fs::read(database).unwrap(), before);
}
#[cfg(unix)]
#[test]
fn an_existing_shared_directory_is_not_silently_chmodded() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let shared = root.path().join("shared");
    std::fs::create_dir(&shared).unwrap();
    std::fs::write(shared.join("unrelated.txt"), b"preserve").unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(DataDirectory::acquire(&shared).is_err());
    assert_eq!(
        std::fs::metadata(&shared).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(
        std::fs::read(shared.join("unrelated.txt")).unwrap(),
        b"preserve"
    );
}
