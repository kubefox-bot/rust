use chrono::NaiveDateTime;
use hmail_cleaner_adapters::mysql::{DatabaseConfig, MySqlRepository};
use sqlx::{Connection, Executor, MySqlConnection};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

static SERIAL: AtomicU64 = AtomicU64::new(0);
const TEST_PORT: u16 = 13306;

pub fn date(value: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").unwrap()
}
pub fn name(id: i64) -> String {
    format!("{{00000000-0000-0000-0000-{id:012X}}}.eml")
}
pub struct Fixture {
    pub admin: MySqlConnection,
    pub config: DatabaseConfig,
    database: String,
    url: Url,
}
impl Fixture {
    pub async fn create() -> Self {
        // Owning configuration boundary for integration fixtures.
        let raw_url = std::env::var("HMAIL_TEST_DATABASE_URL")
            .expect("HMAIL_TEST_DATABASE_URL is required; start the disposable MySQL fixture");
        let mut url = Url::parse(&raw_url).expect("invalid integration fixture URL");
        assert_eq!(url.scheme(), "mysql");
        assert!(
            matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1")),
            "integration fixtures require a loopback server"
        );
        assert_eq!(
            url.port(),
            Some(TEST_PORT),
            "integration fixtures require port 13306"
        );
        assert_eq!(
            url.path(),
            "/mysql",
            "integration fixtures require the mysql bootstrap database"
        );
        let mut admin = MySqlConnection::connect(&raw_url).await.unwrap();
        let version: String = sqlx::query_scalar("SELECT VERSION()")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert!(
            version.starts_with("5.7.21"),
            "fixture must use pinned MySQL 5.7.21"
        );
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = SERIAL.fetch_add(1, Ordering::Relaxed);
        let database = format!("hmail_cleanup_test_{nanos}_{sequence}");
        // Identifiers are generated ASCII, never caller-supplied database names.
        admin
            .execute(format!("CREATE DATABASE {database} CHARACTER SET utf8").as_str())
            .await
            .unwrap();
        admin
            .execute(format!("USE {database}").as_str())
            .await
            .unwrap();
        for statement in include_str!("schema.sql")
            .split(';')
            .filter(|part| !part.trim().is_empty())
        {
            admin.execute(statement).await.unwrap();
        }
        admin.execute(
            "INSERT INTO hm_accounts VALUES
             (1,1,0,'info@chem.orp','',1,0,'','',0,0,'','',0,0,'',0,0,'','','2026-01-01',0,'2026-01-01','',''),
             (2,1,0,'sales@chem.orp','',1,0,'','',0,0,'','',0,0,'',0,0,'','','2026-01-01',0,'2026-01-01','','')"
        ).await.unwrap();
        admin
            .execute(
                "INSERT INTO hm_imapfolders VALUES
             (10,1,0,'INBOX',1,'2026-01-01',100),
             (20,1,0,'Sent',1,'2026-01-01',100),
             (30,2,0,'INBOX',1,'2026-01-01',100),
             (40,0,0,'Public',1,'2026-01-01',100)",
            )
            .await
            .unwrap();
        url.set_path(&database);
        let config = DatabaseConfig::parse(url.as_str()).unwrap();
        Self {
            admin,
            config,
            database,
            url,
        }
    }
    pub async fn add(
        &mut self,
        id: i64,
        account: i32,
        folder: i32,
        state: i8,
        created: &str,
        filename: &str,
    ) {
        sqlx::query(
            "INSERT INTO hm_messages
             (messageid,messageaccountid,messagefolderid,messagefilename,messagetype,messagefrom,
              messagesize,messagecurnooftries,messagenexttrytime,messageflags,messagecreatetime,messagelocked,messageuid)
             VALUES (?,?,?,?,?,'fixture@example.invalid',100,0,'2026-01-01',0,?,0,?)"
        ).bind(id).bind(account).bind(folder).bind(filename).bind(state).bind(date(created)).bind(id)
            .execute(&mut self.admin).await.unwrap();
    }
    pub async fn add_dependents(&mut self, id: i64) {
        sqlx::query(
            "INSERT INTO hm_message_metadata
             (metadata_id,metadata_accountid,metadata_folderid,metadata_messageid,metadata_from,
              metadata_subject,metadata_to,metadata_cc)
             VALUES (?,1,10,?,'fixture@example.invalid','','','')",
        )
        .bind(id)
        .bind(id)
        .execute(&mut self.admin)
        .await
        .unwrap();
        sqlx::query("INSERT INTO hm_messagerecipients VALUES (?,?,'fixture@example.invalid',1,'')")
            .bind(id)
            .bind(id)
            .execute(&mut self.admin)
            .await
            .unwrap();
    }
    pub async fn repository(&self) -> MySqlRepository {
        MySqlRepository::connect(&self.config).await.unwrap()
    }
    pub async fn read_only_config(&mut self) -> (String, DatabaseConfig) {
        let user = format!(
            "hc_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        self.admin
            .execute(format!("CREATE USER '{user}'@'%' IDENTIFIED BY 'fixture_readonly'").as_str())
            .await
            .unwrap();
        self.admin
            .execute(format!("GRANT SELECT ON {}.* TO '{user}'@'%'", self.database).as_str())
            .await
            .unwrap();
        let mut url = self.url.clone();
        url.set_username(&user).unwrap();
        url.set_password(Some("fixture_readonly")).unwrap();
        (user, DatabaseConfig::parse(url.as_str()).unwrap())
    }
    pub async fn drop_reader(&mut self, user: &str) {
        assert!(user.starts_with("hc_") && user[3..].bytes().all(|byte| byte.is_ascii_digit()));
        self.admin
            .execute(format!("DROP USER '{user}'@'%'").as_str())
            .await
            .unwrap();
    }
    pub async fn ids(&mut self, table: &str, column: &str) -> Vec<i64> {
        assert!(matches!(
            (table, column),
            ("hm_messages", "messageid")
                | ("hm_message_metadata", "metadata_messageid")
                | ("hm_messagerecipients", "recipientmessageid")
        ));
        sqlx::query_scalar(&format!("SELECT {column} FROM {table} ORDER BY {column}"))
            .fetch_all(&mut self.admin)
            .await
            .unwrap()
    }
    pub async fn finish(mut self) {
        self.admin
            .execute(format!("DROP DATABASE {}", self.database).as_str())
            .await
            .unwrap();
        self.admin.close().await.unwrap();
    }
}
