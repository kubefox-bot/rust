pub mod fixtures;
use hmail_cleaner_center::{
    identity::IdentityService,
    profiles::{ProfileService, ServerRegistration},
};
use hmail_cleaner_protocol::{
    browser::profiles::{PasswordPatch, ProfilePatch},
    AgentId, DatabaseTls, ProfileId, ServerId,
};
fn patch(password: PasswordPatch) -> ProfilePatch {
    ProfilePatch {
        mysql_host: "127.0.0.1".into(),
        mysql_port: 3306,
        mysql_database: "hmail".into(),
        mysql_username: "hmail".into(),
        password,
        mysql_tls: DatabaseTls::DisabledLocal,
        data_dir: r"C:\Mail Data\Данные".into(),
        hmail_ini: r"C:\hMailServer\hMailServer.ini".into(),
        service_name: "hMailServer".into(),
    }
}
#[tokio::test]
async fn edits_keep_old_versions_preserve_windows_paths_and_never_return_passwords() {
    let fixture = fixtures::center().await;
    let service = ProfileService::new(fixture.store.clone());
    let agent_id = AgentId::new();
    let profile_id = ProfileId::new();
    let server_id = ServerId::new();
    let (_, csr) = fixtures::csr();
    IdentityService::new(fixture.store.clone())
        .issue_agent(agent_id, "127.0.0.1", &csr, &fixture.lease)
        .await
        .unwrap();
    service
        .register(
            ServerRegistration {
                server_id,
                profile_id,
                agent_id,
                name: "Mail One".into(),
                endpoint: "https://127.0.0.1:8444".into(),
            },
            &fixture.lease,
        )
        .await
        .unwrap();
    assert!(
        !service
            .read(profile_id, &fixture.lease)
            .await
            .unwrap()
            .configured
    );
    assert!(service
        .save(profile_id, 1, patch(PasswordPatch::Keep), &fixture.lease)
        .await
        .is_err());
    let version = service
        .save(
            profile_id,
            1,
            patch(PasswordPatch::Replace(zeroize::Zeroizing::new(
                "MYSQL-PASSWORD-SENTINEL".into(),
            ))),
            &fixture.lease,
        )
        .await
        .unwrap();
    assert_eq!(version.version.get(), 2);
    let editable = service.read(profile_id, &fixture.lease).await.unwrap();
    assert_eq!(editable.data_dir, r"C:\Mail Data\Данные");
    assert!(editable.password_set);
    assert!(!serde_json::to_string(&editable)
        .unwrap()
        .contains("MYSQL-PASSWORD-SENTINEL"));
    service
        .save(profile_id, 2, patch(PasswordPatch::Keep), &fixture.lease)
        .await
        .unwrap();
    assert!(service
        .save(profile_id, 2, patch(PasswordPatch::Keep), &fixture.lease)
        .await
        .is_err());
    let old = service
        .for_job(server_id, profile_id, 2, &fixture.lease)
        .await
        .unwrap();
    assert_eq!(
        old.mail_profile.mysql.password.as_str(),
        "MYSQL-PASSWORD-SENTINEL"
    );
    let current = service
        .for_job(server_id, profile_id, 3, &fixture.lease)
        .await
        .unwrap();
    assert_eq!(
        old.mail_profile.mysql.password,
        current.mail_profile.mysql.password
    );
    assert!(service
        .for_job(ServerId::new(), profile_id, 3, &fixture.lease)
        .await
        .is_err());
}
#[tokio::test]
async fn whole_envelope_swap_and_corrupt_storage_cannot_be_read_as_another_profile() {
    use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection};
    let fixture = fixtures::center().await;
    let service = ProfileService::new(fixture.store.clone());
    let (first_server, first_profile) = (ServerId::new(), ProfileId::new());
    let (second_server, second_profile) = (ServerId::new(), ProfileId::new());
    for (server_id, profile_id) in [
        (first_server, first_profile),
        (second_server, second_profile),
    ] {
        let agent_id = AgentId::new();
        let (_, csr) = fixtures::csr();
        IdentityService::new(fixture.store.clone())
            .issue_agent(agent_id, "127.0.0.1", &csr, &fixture.lease)
            .await
            .unwrap();
        service
            .register(
                ServerRegistration {
                    server_id,
                    profile_id,
                    agent_id,
                    name: "fixture".into(),
                    endpoint: "https://127.0.0.1:8444".into(),
                },
                &fixture.lease,
            )
            .await
            .unwrap();
        service
            .save(
                profile_id,
                1,
                patch(PasswordPatch::Replace(zeroize::Zeroizing::new(
                    "fixture-mysql-password".into(),
                ))),
                &fixture.lease,
            )
            .await
            .unwrap();
    }
    let mut database = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(fixture.root.path().join("center.sqlite")),
    )
    .await
    .unwrap();
    let original: String =
        sqlx::query_scalar("SELECT record_id FROM profile_versions WHERE id=? AND version=2")
            .bind(first_profile.to_string())
            .fetch_one(&mut database)
            .await
            .unwrap();
    let other: String =
        sqlx::query_scalar("SELECT record_id FROM profile_versions WHERE id=? AND version=2")
            .bind(second_profile.to_string())
            .fetch_one(&mut database)
            .await
            .unwrap();
    assert!(
        sqlx::query("UPDATE profile_versions SET record_id=? WHERE id=? AND version=2")
            .bind(&other)
            .bind(first_profile.to_string())
            .execute(&mut database)
            .await
            .is_err()
    );
    // Simulate hostile disk metadata rewriting, beyond the application's immutable SQL API.
    sqlx::query("DROP TRIGGER profile_immutable_update")
        .execute(&mut database)
        .await
        .unwrap();
    sqlx::query("UPDATE profile_versions SET record_id=? WHERE id=? AND version=2")
        .bind(&other)
        .bind(first_profile.to_string())
        .execute(&mut database)
        .await
        .unwrap();
    assert!(service
        .for_job(first_server, first_profile, 2, &fixture.lease)
        .await
        .is_err());
    sqlx::query("UPDATE profile_versions SET record_id=? WHERE id=? AND version=2")
        .bind(&original)
        .bind(first_profile.to_string())
        .execute(&mut database)
        .await
        .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints=ON")
        .execute(&mut database)
        .await
        .unwrap();
    sqlx::query("UPDATE encrypted_records SET envelope=zeroblob(1048576) WHERE id=?")
        .bind(original)
        .execute(&mut database)
        .await
        .unwrap();
    assert!(service.read(first_profile, &fixture.lease).await.is_err());
}
#[tokio::test]
async fn http_profile_body_limit_and_authorization_preserve_secret_redaction() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    let fixture = fixtures::center().await;
    let service = ProfileService::new(fixture.store.clone());
    let (agent_id, profile_id, server_id) = (AgentId::new(), ProfileId::new(), ServerId::new());
    let (_, csr) = fixtures::csr();
    IdentityService::new(fixture.store.clone())
        .issue_agent(agent_id, "127.0.0.1", &csr, &fixture.lease)
        .await
        .unwrap();
    service
        .register(
            ServerRegistration {
                server_id,
                profile_id,
                agent_id,
                name: "Mail One".into(),
                endpoint: "https://127.0.0.1:8444".into(),
            },
            &fixture.lease,
        )
        .await
        .unwrap();
    let mut state = hmail_cleaner_center::http::AppState::new(
        fixture.store.clone(),
        hmail_cleaner_center::config::BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap();
    state.authority = fixture.authority.clone();
    let app = hmail_cleaner_center::http::router(state);
    let path = format!("/api/v1/servers/{server_id}/profile");
    let response = app
        .clone()
        .oneshot(
            Request::get(&path)
                .header("host", "localhost:8443")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 401);
    let mut profile = patch(PasswordPatch::Replace(zeroize::Zeroizing::new(
        "HTTP-PASSWORD-SENTINEL".into(),
    )));
    profile.data_dir = "D:\\".to_owned() + &"d".repeat(3000);
    profile.hmail_ini = "C:\\".to_owned() + &"i".repeat(3000);
    profile.mysql_username = "u".repeat(3000);
    let bytes = serde_json::to_vec(
        &hmail_cleaner_protocol::browser::profiles::SaveProfileRequest {
            expected_version: 1.into(),
            profile,
        },
    )
    .unwrap();
    assert!(bytes.len() > 8192);
    let cookie = format!("__Host-hmail-session={}", fixture.token.as_str());
    let response = app
        .clone()
        .oneshot(
            Request::patch(&path)
                .header("host", "localhost:8443")
                .header("x-hmail-request", "1")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let response = app
        .oneshot(
            Request::get(&path)
                .header("host", "localhost:8443")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let body = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    assert!(!body
        .windows("HTTP-PASSWORD-SENTINEL".len())
        .any(|w| w == b"HTTP-PASSWORD-SENTINEL"));
}
