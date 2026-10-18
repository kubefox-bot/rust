use axum::{body::Body, http::Request};
#[cfg(feature = "cli")]
use hmail_cleaner_center::config::CenterArgs;
use hmail_cleaner_center::{
    account::{initialize, InitInput, SecretPrompt},
    app,
    config::{BrowserOrigin, CenterConfig},
    http::{router, AppState},
    storage::{CenterStore, DataDirectory},
    vault::SecretText,
};
use std::{path::Path, sync::Arc, time::Duration};
use tower::ServiceExt;

struct Prompt;
impl SecretPrompt for Prompt {
    fn password(&mut self) -> Result<SecretText, String> {
        Ok(SecretText::new("fixture-delivery-password".into()))
    }
    fn confirm_recovery(&mut self, _: &SecretText) -> Result<bool, String> {
        Ok(true)
    }
}
async fn initialized(path: &Path) -> CenterStore {
    let directory = DataDirectory::acquire(path).unwrap();
    let store = CenterStore::open(&directory).await.unwrap();
    initialize(
        &store,
        &InitInput {
            login: "admin".into(),
        },
        &mut Prompt,
    )
    .await
    .unwrap();
    store
}
fn client(path: &Path) -> reqwest::Client {
    let mut roots = rustls::RootCertStore::empty();
    let pem = std::fs::read(path.join("ca-cert.pem")).unwrap();
    for certificate in rustls_pemfile::certs(&mut std::io::Cursor::new(pem)) {
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
        .timeout(Duration::from_secs(10))
        .tls_backend_preconfigured(tls)
        .build()
        .unwrap()
}

#[tokio::test]
async fn center_health_before_login_reports_exact_build_and_schema_without_private_data() {
    let root = tempfile::tempdir().unwrap();
    let store = initialized(root.path()).await;
    let state = AppState::new(
        store,
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap()
    .with_build_version("delivery-fixture.123")
    .unwrap();
    let response = router(state)
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["content-type"], "application/json");
    assert!(!response.headers().contains_key("set-cookie"));
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({
            "status":"ok", "build_version":"delivery-fixture.123", "schema_version":2
        })
    );
}

#[tokio::test]
async fn shutdown_revokes_session_before_draining_an_open_event_stream() {
    let root = tempfile::tempdir().unwrap();
    let store = initialized(root.path()).await;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!(
        "https://localhost:{}",
        listener.local_addr().unwrap().port()
    );
    let state = AppState::new(store.clone(), BrowserOrigin::parse(&origin).unwrap())
        .await
        .unwrap();
    let account = store.account().await.unwrap().unwrap();
    let dek = hmail_cleaner_center::vault::open_password(
        &account.material,
        &SecretText::new("fixture-delivery-password".into()),
    )
    .unwrap();
    let grant = state.authority().admit(account.revision, dek).unwrap();
    let lease = state.authority().authenticate(&grant.token).unwrap();
    let mut revoked = lease.revocation();
    let config = CenterConfig {
        data_dir: root.path().to_path_buf(),
        listen: listener.local_addr().unwrap(),
        origin: BrowserOrigin::parse(&origin).unwrap(),
        static_dir: root.path().to_path_buf(),
    };
    let cancel = Arc::new(hmail_cleaner_core::cleanup::Cancellation::default());
    let server_cancel = cancel.clone();
    let server =
        tokio::spawn(async move { app::serve(listener, &config, state, server_cancel).await });
    let mut stream = client(root.path())
        .get(format!(
            "{origin}/api/v1/events?server_id={}",
            uuid::Uuid::new_v4()
        ))
        .header(
            "cookie",
            format!("__Host-hmail-session={}", grant.token.as_str()),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(stream.status().as_u16(), 200);
    assert!(stream.chunk().await.unwrap().is_some());
    cancel.request();
    tokio::time::timeout(Duration::from_secs(3), revoked.changed())
        .await
        .expect("session must be revoked before the five-second drain")
        .unwrap();
    assert!(!lease.is_live());
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[cfg(feature = "cli")]
#[tokio::test]
async fn health_cli_uses_verified_local_https_without_acquiring_the_data_lock() {
    for origin in ["https://delivery-health.invalid", "https://192.0.2.9"] {
        let root = tempfile::tempdir().unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let listen = format!("0.0.0.0:{}", listener.local_addr().unwrap().port())
            .parse()
            .unwrap();
        let directory = DataDirectory::acquire(root.path()).unwrap();
        CenterConfig::create(root.path(), origin, listen).unwrap();
        drop(directory);
        let store = initialized(root.path()).await;
        let config = CenterConfig::load(&CenterArgs {
            data_dir: root.path().to_path_buf(),
        })
        .unwrap();
        let state = AppState::new(store, config.origin.clone())
            .await
            .unwrap()
            .with_build_version("delivery-fixture.123")
            .unwrap();
        let cancel = Arc::new(hmail_cleaner_core::cleanup::Cancellation::default());
        let server_cancel = cancel.clone();
        let server =
            tokio::spawn(async move { app::serve(listener, &config, state, server_cancel).await });
        assert!(DataDirectory::acquire(root.path()).is_err());
        let command = |expected: &str| {
            [
                "health".into(),
                "--data-dir".into(),
                root.path().as_os_str().to_os_string(),
                "--expect-build".into(),
                expected.into(),
            ]
        };
        assert_eq!(
            hmail_cleaner_center::cli::run(
                command("delivery-fixture.123").into_iter(),
                "delivery-fixture.123"
            )
            .await,
            std::process::ExitCode::SUCCESS
        );
        assert_eq!(
            hmail_cleaner_center::cli::run(
                command("different-build").into_iter(),
                "delivery-fixture.123"
            )
            .await,
            std::process::ExitCode::FAILURE
        );
        let config_file = root.path().join("center.json");
        let saved = std::fs::read(&config_file).unwrap();
        let mut wrong_name: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        wrong_name["origin"] = "https://different-name.invalid".into();
        std::fs::write(&config_file, serde_json::to_vec(&wrong_name).unwrap()).unwrap();
        assert_eq!(
            hmail_cleaner_center::cli::run(
                command("delivery-fixture.123").into_iter(),
                "delivery-fixture.123"
            )
            .await,
            std::process::ExitCode::FAILURE
        );
        std::fs::write(&config_file, saved).unwrap();
        let other =
            rcgen::generate_simple_self_signed(vec!["unrelated-ca.invalid".into()]).unwrap();
        std::fs::write(root.path().join("ca-cert.pem"), other.cert.pem()).unwrap();
        assert_eq!(
            hmail_cleaner_center::cli::run(
                command("delivery-fixture.123").into_iter(),
                "delivery-fixture.123"
            )
            .await,
            std::process::ExitCode::FAILURE
        );
        cancel.request();
        server.await.unwrap().unwrap();
    }
}

#[cfg(feature = "cli")]
#[tokio::test]
async fn backup_cli_requires_a_stopped_initialized_center_and_preserves_identity_and_build() {
    let root = tempfile::tempdir().unwrap();
    let store = initialized(root.path()).await;
    let before = store.account().await.unwrap().unwrap().material.vault_id();
    let target = tempfile::tempdir().unwrap();
    let destination = target.path().join("snapshot");
    let invalid_destination = target.path().join("invalid-build");
    assert!(store
        .backup_with_build(&invalid_destination, "unsafe\nversion")
        .await
        .is_err());
    assert!(!invalid_destination.exists());
    let command = || {
        [
            "backup".into(),
            "--data-dir".into(),
            root.path().as_os_str().to_os_string(),
            "--destination".into(),
            destination.as_os_str().to_os_string(),
        ]
    };
    assert_eq!(
        hmail_cleaner_center::cli::run(command().into_iter(), "delivery-fixture.123").await,
        std::process::ExitCode::FAILURE
    );
    assert!(!destination.exists());
    drop(store);
    assert_eq!(
        hmail_cleaner_center::cli::run(command().into_iter(), "delivery-fixture.123").await,
        std::process::ExitCode::SUCCESS
    );
    let manifest: hmail_cleaner_center::storage::BackupManifest =
        serde_json::from_slice(&std::fs::read(destination.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest.vault_id, Some(before));
    assert_eq!(manifest.build_version, "delivery-fixture.123");
    assert_eq!(manifest.schema_version, 2);
    for name in ["https-cert.pem", "https-key.pem", "ca-cert.pem"] {
        assert_eq!(
            std::fs::read(root.path().join(name)).unwrap(),
            std::fs::read(destination.join(name)).unwrap()
        );
    }
    assert_eq!(
        hmail_cleaner_center::cli::run(command().into_iter(), "delivery-fixture.123").await,
        std::process::ExitCode::FAILURE
    );
    let lease = DataDirectory::acquire(&destination).unwrap();
    let restored = CenterStore::open(&lease).await.unwrap();
    assert_eq!(
        restored
            .account()
            .await
            .unwrap()
            .unwrap()
            .material
            .vault_id(),
        before
    );
}
