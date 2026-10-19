pub mod fixtures;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use hmail_cleaner_center::{
    config::BrowserOrigin,
    http::{router, static_files::StaticDir, AppState},
};
use tower::ServiceExt;

fn web_build() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    for page in ["", "login", "history", "settings"] {
        let directory = root.path().join(page);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("index.html"),
            "<!doctype html><html data-header-profile=\"guest\"><body>Header</body></html>",
        )
        .unwrap();
    }
    std::fs::create_dir(root.path().join("_astro")).unwrap();
    std::fs::write(root.path().join("_astro/app.js"), "export const okay=true").unwrap();
    root
}
async fn page(app: &Router, cookie: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .uri("/settings/")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn html(response: axum::response::Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}
#[tokio::test]
async fn html_header_requires_live_session_and_issues_opaque_httponly_profile() {
    let f = fixtures::center().await;
    let build = web_build();
    let state = AppState::new(
        f.store.clone(),
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap()
    .with_authority(f.authority.clone())
    .with_static_dir(StaticDir::open(build.path()).unwrap());
    let app = router(state);
    assert!(html(page(&app, "").await)
        .await
        .contains("data-header-profile=\"guest\""));
    let session = format!("__Host-hmail-session={}", f.token.as_str());
    let response = page(&app, &session).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let profile = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("__Host-sochi-profile="))
        .expect("encrypted profile cookie")
        .to_string();
    assert!(profile.contains("HttpOnly"));
    assert!(profile.contains("Secure"));
    assert!(profile.contains("SameSite=Strict"));
    assert!(profile.contains("Path=/"));
    assert!(!profile.contains("admin"));
    assert!(html(response)
        .await
        .contains("data-header-profile=\"admin\""));
    let cookie = profile.split(';').next().unwrap();
    let both = format!("{session}; {cookie}");
    let head = app
        .clone()
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri("/settings/")
                .header("cookie", &both)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::OK);
    assert_eq!(head.headers()["cache-control"], "no-store");
    assert!(head.headers().get("content-length").is_some());
    assert!(to_bytes(head.into_body(), 4096).await.unwrap().is_empty());
    for (header, value) in [
        ("range", "bytes=0-10"),
        ("if-none-match", "*"),
        ("if-modified-since", "Wed, 21 Oct 2037 07:28:00 GMT"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/settings/")
                    .header("cookie", &both)
                    .header(header, value)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(response.headers().get("etag").is_none());
        assert!(html(response)
            .await
            .contains("data-header-profile=\"admin\""));
    }
    assert!(html(page(&app, cookie).await)
        .await
        .contains("data-header-profile=\"guest\""));
    let response = page(&app, &both).await;
    assert!(response.headers().get("set-cookie").is_none());
    assert!(html(response)
        .await
        .contains("data-header-profile=\"admin\""));
    let forged = format!("{session}; __Host-sochi-profile=eyJhbGciOiJIUzI1NiJ9.invalid.signature");
    let repaired = page(&app, &forged).await;
    assert!(repaired.headers().get("set-cookie").is_some());
    assert!(html(repaired)
        .await
        .contains("data-header-profile=\"admin\""));
    let asset = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/_astro/app.js")
                .header("cookie", &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(asset.headers().get("set-cookie").is_none());
    assert_eq!(
        asset.headers()["cache-control"],
        "public, max-age=31536000, immutable"
    );
    f.authority.revoke(f.lease.generation());
    assert!(html(page(&app, &both).await)
        .await
        .contains("data-header-profile=\"guest\""));
    let protected = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/servers")
                .header("host", "localhost:8443")
                .header("cookie", &both)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(protected.status(), StatusCode::UNAUTHORIZED);
}
