use hmail_cleaner_center::{
    app,
    config::{BrowserOrigin, CenterConfig},
    storage::DataDirectory,
};
use hmail_cleaner_core::cleanup::Cancellation;
use std::sync::Arc;

#[tokio::test]
async fn unusable_journal_fails_startup_before_opening_the_database() {
    let root = tempfile::tempdir().unwrap();
    drop(DataDirectory::acquire(root.path()).unwrap());
    std::fs::create_dir(root.path().join("center.ndjson")).unwrap();
    let error = app::run(
        CenterConfig {
            data_dir: root.path().into(),
            listen: "127.0.0.1:0".parse().unwrap(),
            origin: BrowserOrigin::parse("https://localhost:8443").unwrap(),
            static_dir: root.path().into(),
        },
        Arc::new(Cancellation::default()),
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().contains("journal"),
        "startup must identify the journal boundary: {error}"
    );
    assert!(!root.path().join("center.sqlite").exists());
}

struct Prompt;
impl hmail_cleaner_center::account::SecretPrompt for Prompt {
    fn password(&mut self) -> Result<hmail_cleaner_center::vault::SecretText, String> {
        Ok(hmail_cleaner_center::vault::SecretText::new(
            "journal-PASSWORD-CANARY".into(),
        ))
    }
    fn confirm_recovery(
        &mut self,
        _: &hmail_cleaner_center::vault::SecretText,
    ) -> Result<bool, String> {
        Ok(true)
    }
}
async fn initialize(root: &std::path::Path) {
    let lease = DataDirectory::acquire(root).unwrap();
    let store = hmail_cleaner_center::storage::CenterStore::open(&lease)
        .await
        .unwrap();
    hmail_cleaner_center::account::initialize(
        &store,
        &hmail_cleaner_center::account::InitInput {
            login: "admin".into(),
        },
        &mut Prompt,
    )
    .await
    .unwrap();
}
fn client(root: &std::path::Path) -> reqwest::Client {
    let mut roots = rustls::RootCertStore::empty();
    for certificate in rustls_pemfile::certs(&mut std::io::Cursor::new(
        std::fs::read(root.join("ca-cert.pem")).unwrap(),
    )) {
        roots.add(certificate.unwrap()).unwrap();
    }
    let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(2))
        .tls_backend_preconfigured(tls)
        .build()
        .unwrap()
}
async fn ready(client: &reqwest::Client, origin: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if client
                .get(format!("{origin}/healthz"))
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn production_https_journal_appends_across_restart_and_excludes_request_secrets() {
    let root = tempfile::tempdir().unwrap();
    initialize(root.path()).await;
    let static_dir = root.path().join("web");
    for page in [
        "index.html",
        "login/index.html",
        "history/index.html",
        "settings/index.html",
    ] {
        let path = static_dir.join(page);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "<html>fixture</html>").unwrap();
    }
    let client = client(root.path());
    for _ in 0..2 {
        let address = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let origin = format!("https://localhost:{}", address.port());
        let config = CenterConfig {
            data_dir: root.path().into(),
            static_dir: static_dir.clone(),
            listen: address,
            origin: BrowserOrigin::parse(&origin).unwrap(),
        };
        let cancel = Arc::new(Cancellation::default());
        let activity = async {
            ready(&client, &origin).await;
            assert!(DataDirectory::acquire(root.path()).is_err());
            let response = client
                .get(format!(
                    "{origin}/api/v1/settings/session?password=URI-CANARY"
                ))
                .header("cookie", "__Host-hmail-session=COOKIE-CANARY")
                .header("authorization", "Bearer AUTH-CANARY")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 401);
            cancel.request();
        };
        let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            tokio::join!(app::run(config, cancel.clone()), activity)
        })
        .await
        .unwrap();
        result.unwrap();
    }
    let text = std::fs::read_to_string(root.path().join("center.ndjson")).unwrap();
    assert!(!text.contains("CANARY"));
    assert!(!text.contains("https://"));
    assert!(!text.contains("PRIVATE KEY"));
    let events = text
        .lines()
        .map(|line| {
            assert!(line.len() < 2048);
            serde_json::from_str::<serde_json::Value>(line).unwrap()["event"].clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        events
            .iter()
            .filter(|event| event["code"] == "service_starting")
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event["code"] == "service_stopped")
            .count(),
        2
    );
    let requests = events
        .iter()
        .filter(|event| event["code"] == "request_failed")
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 2);
    for request in requests {
        assert_eq!(request["status"], 401);
        assert_eq!(request["operation"], "settings");
        uuid::Uuid::parse_str(request["correlation_id"].as_str().unwrap()).unwrap();
    }
}

#[tokio::test]
async fn request_write_fault_preserves_http_error_and_exposes_unhealthy_status() {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    let store = hmail_cleaner_center::storage::CenterStore::open(&lease)
        .await
        .unwrap();
    hmail_cleaner_center::account::initialize(
        &store,
        &hmail_cleaner_center::account::InitInput {
            login: "admin".into(),
        },
        &mut Prompt,
    )
    .await
    .unwrap();
    let path = root.path().join("center.ndjson");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(10 * 1024 * 1024)
        .unwrap();
    let journal = hmail_cleaner_adapters::journal::RuntimeJournal::open(&path).unwrap();
    std::fs::create_dir(root.path().join("center.ndjson.4")).unwrap();
    let state = app::AppState::new(
        store,
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap()
    .with_journal(journal.clone());
    let router = hmail_cleaner_center::http::router(state);
    let response = router
        .clone()
        .oneshot(
            Request::get("/api/v1/settings/session")
                .header("host", "localhost:8443")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 401);
    assert!(!journal.is_healthy());
    let response = router
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 503);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["status"],
        "unavailable"
    );
}
