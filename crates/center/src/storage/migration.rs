use super::StorageError;
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;

pub(super) async fn upgrade(pool: &SqlitePool) -> Result<(), StorageError> {
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await
        .map_err(|_| StorageError::Database)?;
    if version == super::connection::SCHEMA_VERSION {
        return Ok(());
    }
    let mut transaction = pool.begin().await.map_err(|_| StorageError::Database)?;
    sqlx::raw_sql(include_str!("../../migrations/0002_server_slugs.sql"))
        .execute(&mut *transaction)
        .await
        .map_err(|_| StorageError::Database)?;
    let servers = sqlx::query("SELECT id,name FROM servers ORDER BY id")
        .fetch_all(&mut *transaction)
        .await
        .map_err(|_| StorageError::Database)?;
    let mut reserved = HashSet::new();
    for server in servers {
        let id: String = server.try_get("id").map_err(|_| StorageError::Database)?;
        let name: String = server.try_get("name").map_err(|_| StorageError::Database)?;
        let slug = crate::profiles::slug::allocate(&name, &reserved);
        sqlx::query("UPDATE servers SET slug=? WHERE id=?")
            .bind(&slug)
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| StorageError::Database)?;
        reserved.insert(slug);
    }
    transaction
        .commit()
        .await
        .map_err(|_| StorageError::Database)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn old_servers_receive_unique_persistent_slugs_and_reopening_keeps_them() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../../migrations/0001_center.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO servers(id,name,agent_id) VALUES('one','Mail One','agent-one'),('two','Mail One','agent-two')").execute(&pool).await.unwrap();
        upgrade(&pool).await.unwrap();
        let slugs: Vec<String> = sqlx::query_scalar("SELECT slug FROM servers ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(slugs, vec!["mail-one", "mail-one-2"]);
        sqlx::query("UPDATE servers SET name='Renamed' WHERE id='one'")
            .execute(&pool)
            .await
            .unwrap();
        upgrade(&pool).await.unwrap();
        let retained: String = sqlx::query_scalar("SELECT slug FROM servers WHERE id='one'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(retained, "mail-one");
    }
}
