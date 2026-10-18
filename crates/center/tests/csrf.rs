use axum::{body::Body, http::Request};
use hmail_cleaner_center::{
    account::{initialize, InitInput, SecretPrompt},
    config::{BrowserOrigin, CenterArgs, CenterConfig},
    http::{router, AppState},
    storage::{CenterStore, DataDirectory},
    vault::SecretText,
};
use tower::ServiceExt;
struct Prompt;
struct ManualClock(std::sync::atomic::AtomicU64);
impl hmail_cleaner_center::auth::Clock for ManualClock {
    fn now(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.0.load(std::sync::atomic::Ordering::SeqCst))
    }
}
impl SecretPrompt for Prompt {
    fn password(&mut self) -> Result<SecretText, String> {
        Ok(SecretText::new("test-session-password".into()))
    }
    fn confirm_recovery(&mut self, _: &SecretText) -> Result<bool, String> {
        Ok(true)
    }
}
#[tokio::test]
async fn unsafe_requests_require_exact_origin_host_custom_header_and_json() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    let store = CenterStore::open(&lease).await.unwrap();
    initialize(
        &store,
        &InitInput {
            login: "admin".into(),
        },
        &mut Prompt,
    )
    .await
    .unwrap();
    let state = AppState::new(
        store,
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap();
    let app = router(state);
    for (host, origin, custom, content, fetch, expected) in [
        ("localhost:8443", None, None, "application/json", None, 403),
        ("evil.test", None, Some("1"), "application/json", None, 403),
        (
            "localhost:8443",
            Some("https://evil.test"),
            Some("1"),
            "application/json",
            Some("same-origin"),
            403,
        ),
        ("localhost:8443", None, Some("1"), "text/plain", None, 415),
        (
            "localhost:8443",
            None,
            Some("1"),
            "application/json",
            Some("cross-site"),
            403,
        ),
        (
            "localhost:8443",
            None,
            Some("1"),
            "application/json",
            None,
            401,
        ),
    ] {
        let builder = Request::post("/api/v1/auth/logout")
            .header("host", host)
            .header("content-type", content);
        let builder = if let Some(value) = origin {
            builder.header("origin", value)
        } else {
            builder
        };
        let builder = if let Some(value) = custom {
            builder.header("x-hmail-request", value)
        } else {
            builder
        };
        let builder = if let Some(value) = fetch {
            builder.header("sec-fetch-site", value)
        } else {
            builder
        };
        let response = app
            .clone()
            .oneshot(builder.body(Body::from("{}")).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), expected);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
}
#[tokio::test]
async fn real_https_login_cookie_failed_login_replacement_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let origin = format!("https://localhost:{port}");
    CenterConfig::create(root.path(), &origin, listener.local_addr().unwrap()).unwrap();
    let config = CenterConfig::load(&CenterArgs {
        data_dir: root.path().to_path_buf(),
    })
    .unwrap();
    let store = CenterStore::open(&lease).await.unwrap();
    initialize(
        &store,
        &InitInput {
            login: "admin".into(),
        },
        &mut Prompt,
    )
    .await
    .unwrap();
    let mut state = AppState::new(store.clone(), config.origin.clone())
        .await
        .unwrap();
    let clock = std::sync::Arc::new(ManualClock(std::sync::atomic::AtomicU64::new(0)));
    state = state.with_authority(std::sync::Arc::new(
        hmail_cleaner_center::auth::SessionAuthority::new(1, 15, clock.clone()).unwrap(),
    ));
    let authority = state.authority().clone();
    let cancel = std::sync::Arc::new(hmail_cleaner_core::cleanup::Cancellation::default());
    let server_cancel = cancel.clone();
    let server = tokio::spawn(async move {
        hmail_cleaner_center::app::serve(listener, &config, state, server_cancel)
            .await
            .unwrap();
    });
    let ca = std::fs::read(root.path().join("ca-cert.pem")).unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(20))
        .tls_backend_preconfigured(
            rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates({
                let mut roots = rustls::RootCertStore::empty();
                for cert in rustls_pemfile::certs(&mut std::io::Cursor::new(ca)) {
                    roots.add(cert.unwrap()).unwrap();
                }
                roots
            })
            .with_no_client_auth(),
        )
        .build()
        .unwrap();
    assert!(client
        .get(format!("{origin}/healthz"))
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    for (host, foreign_origin) in [
        (format!("localhost:{port}"), "https://evil.test"),
        ("evil.test".into(), origin.as_str()),
    ] {
        let rejected = client
            .post(format!("{origin}/api/v1/auth/login"))
            .header("host", host)
            .header("origin", foreign_origin)
            .header("x-hmail-request", "1")
            .json(&serde_json::json!({"login":"admin","password":"test-session-password"}))
            .send()
            .await
            .unwrap();
        assert_eq!(rejected.status().as_u16(), 403);
        assert_eq!(rejected.headers()["cache-control"], "no-store");
    }
    let login = client
        .post(format!("{origin}/api/v1/auth/login"))
        .header("x-hmail-request", "1")
        .header("origin", &origin)
        .json(&serde_json::json!({"login":"admin","password":"test-session-password"}))
        .send()
        .await
        .unwrap();
    assert!(login.status().is_success(), "{}", login.status());
    let cookie = login
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("__Host-hmail-session="))
        .unwrap()
        .to_owned();
    for attribute in ["Secure", "HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(cookie.contains(attribute));
    }
    assert!(!cookie.contains("Domain="));
    assert!(!cookie.contains("Max-Age="));
    let token = cookie.split(';').next().unwrap();
    let failure = client
        .post(format!("{origin}/api/v1/auth/login"))
        .header("x-hmail-request", "1")
        .json(&serde_json::json!({"login":"admin","password":"wrong-long-password"}))
        .send()
        .await
        .unwrap();
    assert_eq!(failure.status().as_u16(), 401);
    let response = client
        .get(format!("{origin}/api/v1/auth/session"))
        .header("cookie", token)
        .send()
        .await
        .unwrap();
    let value: serde_json::Value = response.json().await.unwrap();
    assert_eq!(value["authenticated"], true);
    let second = client
        .post(format!("{origin}/api/v1/auth/login"))
        .header("x-hmail-request", "1")
        .json(&serde_json::json!({"login":"admin","password":"test-session-password"}))
        .send()
        .await
        .unwrap();
    assert!(second.status().is_success());
    let response = client
        .get(format!("{origin}/api/v1/auth/session"))
        .header("cookie", token)
        .send()
        .await
        .unwrap();
    let value: serde_json::Value = response.json().await.unwrap();
    assert_eq!(value["authenticated"], false);
    let restarted = AppState::new(store, BrowserOrigin::parse(&origin).unwrap())
        .await
        .unwrap();
    let second_cookie = second
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("__Host-hmail-session="))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .split_once('=')
        .unwrap()
        .1;
    assert!(restarted.authority().authenticate(second_cookie).is_err());
    let mut revoked = authority.authenticate(second_cookie).unwrap().revocation();
    clock.0.store(901, std::sync::atomic::Ordering::SeqCst);
    tokio::time::timeout(std::time::Duration::from_secs(3), revoked.changed())
        .await
        .unwrap()
        .unwrap();
    assert!(*revoked.borrow());
    cancel.request();
    server.await.unwrap();
}
#[tokio::test]
async fn slow_body_times_out_before_login_work() {
    let root = tempfile::tempdir().unwrap();
    let lease = DataDirectory::acquire(root.path()).unwrap();
    let store = CenterStore::open(&lease).await.unwrap();
    initialize(
        &store,
        &InitInput {
            login: "admin".into(),
        },
        &mut Prompt,
    )
    .await
    .unwrap();
    let state = AppState::new(
        store,
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap();
    tokio::time::pause();

    let body = Body::from_stream(futures_util::stream::pending::<
        Result<axum::body::Bytes, std::io::Error>,
    >());
    let request = Request::post("/api/v1/auth/login")
        .header("host", "localhost:8443")
        .header("x-hmail-request", "1")
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    let response = router(state).oneshot(request).await.unwrap();
    assert_eq!(response.status().as_u16(), 408);
    assert_eq!(response.headers()["connection"], "close");
    assert_eq!(response.headers()["cache-control"], "no-store");
}

#[tokio::test]
async fn encrypted_httponly_profile_cookie_cannot_authorize_requests() {
    let root = tempfile::tempdir().unwrap();
    let directory = DataDirectory::acquire(root.path()).unwrap();
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
    let state = AppState::new(
        store,
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap();
    let app = router(state);
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/auth/login")
                .header("host", "localhost:8443")
                .header("content-type", "application/json")
                .header("x-hmail-request", "1")
                .body(Body::from(
                    r#"{"login":"admin","password":"test-session-password"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let cookies: Vec<_> = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap().to_owned())
        .collect();
    let profile = cookies
        .iter()
        .find(|v| v.starts_with("__Host-sochi-profile="))
        .expect("login must issue encrypted display profile");
    assert!(profile.contains("Secure"));
    assert!(profile.contains("SameSite=Strict"));
    assert!(profile.contains("Path=/"));
    assert!(profile.contains("HttpOnly"));
    assert!(profile.starts_with("__Host-sochi-profile=v1."));
    let session = cookies
        .iter()
        .find(|v| v.starts_with("__Host-hmail-session="))
        .unwrap();
    assert!(session.contains("HttpOnly"));
    let profile_pair = profile.split(';').next().unwrap();
    let denied = app
        .clone()
        .oneshot(
            Request::get("/api/v1/servers")
                .header("host", "localhost:8443")
                .header("cookie", profile_pair)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), axum::http::StatusCode::UNAUTHORIZED);
    let authenticated = app
        .clone()
        .oneshot(
            Request::get("/api/v1/auth/session")
                .header("host", "localhost:8443")
                .header("cookie", session.split(';').next().unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(authenticated
        .headers()
        .get_all("set-cookie")
        .iter()
        .any(|v| v.to_str().unwrap().starts_with("__Host-sochi-profile=")));
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(authenticated.into_body(), 4096)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["authenticated"], true);
    let logout = app
        .oneshot(
            Request::post("/api/v1/auth/logout")
                .header("host", "localhost:8443")
                .header("content-type", "application/json")
                .header("x-hmail-request", "1")
                .header(
                    "cookie",
                    format!("{}; {}", session.split(';').next().unwrap(), profile_pair),
                )
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout.status(), axum::http::StatusCode::NO_CONTENT);
    assert!(logout.headers().get_all("set-cookie").iter().any(|value| {
        let cookie = value.to_str().unwrap();
        cookie.starts_with("__Host-sochi-profile=") && cookie.contains("Max-Age=0")
    }));
}
