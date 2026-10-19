use hmail_cleaner_protocol::DatabaseParts;
use sqlx::{Connection, Executor, MySqlConnection};
use url::Url;

pub struct Database {
    pub admin: MySqlConnection,
    pub parts: DatabaseParts,
    name: String,
}
impl Database {
    pub async fn create() -> Self {
        // Owning environment boundary; refuses any target outside the disposable fixture.
        let raw = std::env::var("HMAIL_TEST_DATABASE_URL")
            .expect("start mise run db:up and configure the disposable fixture");
        let url = Url::parse(&raw).unwrap();
        assert_eq!(url.scheme(), "mysql");
        assert!(matches!(
            url.host_str(),
            Some("127.0.0.1" | "localhost" | "::1")
        ));
        assert_eq!(url.port(), Some(13306));
        assert_eq!(url.path(), "/mysql");
        assert_eq!(url.username(), "root");
        assert_eq!(
            url.password(),
            Some("hmail_fixture"),
            "only the documented synthetic fixture credentials are supported"
        );
        let mut admin = MySqlConnection::connect(&raw).await.unwrap();
        let version: String = sqlx::query_scalar("SELECT VERSION()")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert!(version.starts_with("5.7.21"));
        let name = format!("hmail_agent_test_{}", uuid::Uuid::new_v4().simple());
        admin
            .execute(format!("CREATE DATABASE {name} CHARACTER SET utf8").as_str())
            .await
            .unwrap();
        admin.execute(format!("USE {name}").as_str()).await.unwrap();
        // Shared canonical schema, without importing another adapter's private test code.
        for statement in include_str!("../../../adapters/tests/fixtures/schema.sql")
            .split(';')
            .filter(|value| !value.trim().is_empty())
        {
            admin.execute(statement).await.unwrap();
        }
        Self {
            parts: DatabaseParts {
                host: url.host_str().unwrap().into(),
                port: 13306,
                database: name.clone(),
                username: url.username().into(),
                password: url.password().unwrap().to_owned().into(),
                tls: hmail_cleaner_protocol::DatabaseTls::DisabledLocal,
            },
            admin,
            name,
        }
    }
    pub async fn finish(mut self) {
        self.admin
            .execute(format!("DROP DATABASE {}", self.name).as_str())
            .await
            .unwrap();
        self.admin.close().await.unwrap();
    }
}
