use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use hmail_cleaner_center::http::static_files::{routes, StaticDir};
use tower::ServiceExt;
#[tokio::test]
async fn static_pages_assets_cache_and_api_boundaries() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("_astro")).unwrap();
    std::fs::create_dir(root.path().join("data")).unwrap();
    std::fs::write(root.path().join("center.sqlite"), "PRIVATE-DATA-SENTINEL").unwrap();
    std::fs::write(
        root.path().join("data/center.sqlite"),
        "PRIVATE-DATA-SENTINEL",
    )
    .unwrap();
    for page in ["index", "login", "history", "settings"] {
        let directory = if page == "index" {
            root.path().to_path_buf()
        } else {
            let dir = root.path().join(page);
            std::fs::create_dir(&dir).unwrap();
            dir
        };
        std::fs::write(
            directory.join("index.html"),
            format!("<!doctype html><title>{page}</title>"),
        )
        .unwrap();
    }
    std::fs::write(
        root.path().join("_astro/app.abcdef.js"),
        "export const okay=true;",
    )
    .unwrap();
    let app = routes::<()>(StaticDir::open(root.path()).unwrap());
    for path in [
        "/",
        "/login",
        "/history",
        "/settings",
        "/history/",
        "/settings/",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(response.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains("text/html"));
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/_astro/app.abcdef.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["cache-control"],
        "public, max-age=31536000, immutable"
    );
    for path in [
        "/api/v1/missing",
        "/data/center.sqlite",
        "/unknown",
        "/_astro/%2e%2e/center.sqlite",
        "/_astro/%252e%252e/center.sqlite",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(!response.status().is_success(), "{path}");
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        assert!(!bytes.starts_with(b"<!doctype html>"));
        assert!(!bytes
            .windows(b"PRIVATE-DATA-SENTINEL".len())
            .any(|part| part == b"PRIVATE-DATA-SENTINEL"));
    }
}
