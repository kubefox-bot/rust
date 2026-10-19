use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use hmail_cleaner_center::http::static_files::{routes, StaticDir};
use tower::ServiceExt;

fn web_fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("_astro")).unwrap();
    for directory in ["", "login", "history", "settings"] {
        let path = root.path().join(directory);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join("index.html"),
            "<!doctype html><title>Dashboard</title>",
        )
        .unwrap();
    }
    root
}

#[tokio::test]
async fn immutable_assets_negotiate_sidecars_with_original_mime_and_plain_fallback() {
    let root = web_fixture();
    for (name, plain) in [
        ("app.abcdef.js", "export const okay=true;"),
        ("app.abcdef.css", "body{color:red}"),
    ] {
        std::fs::write(root.path().join("_astro").join(name), plain).unwrap();
        std::fs::write(
            root.path().join("_astro").join(format!("{name}.br")),
            b"brotli fixture",
        )
        .unwrap();
        std::fs::write(
            root.path().join("_astro").join(format!("{name}.gz")),
            b"gzip fixture",
        )
        .unwrap();
    }
    let app = routes::<()>(StaticDir::open(root.path()).unwrap());
    for (accept, encoding, expected) in [
        ("br, gzip", Some("br"), "brotli fixture"),
        ("br;q=0.2, gzip;q=0.8", Some("gzip"), "gzip fixture"),
        ("br;q=0, gzip;q=0", None, "export const okay=true;"),
        ("identity", None, "export const okay=true;"),
        ("", None, "export const okay=true;"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/_astro/app.abcdef.js")
                    .header("accept-encoding", accept)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{accept}");
        assert_eq!(
            response
                .headers()
                .get("content-encoding")
                .map(|value| value.to_str().unwrap()),
            encoding,
            "{accept}"
        );
        assert_eq!(response.headers()["vary"], "accept-encoding");
        assert_eq!(
            response.headers()["cache-control"],
            "public, max-age=31536000, immutable"
        );
        assert!(response.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains("javascript"));
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            expected
        );
    }
    let css = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/_astro/app.abcdef.css")
                .header("accept-encoding", "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(css.headers()["content-type"], "text/css");
    assert_eq!(css.headers()["content-encoding"], "gzip");
    let invalid_range = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/_astro/app.abcdef.js")
                .header("range", "bytes=999999-1000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_range.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert!(!invalid_range.headers().contains_key("cache-control"));
    std::fs::remove_file(root.path().join("_astro/app.abcdef.js.br")).unwrap();
    std::fs::remove_file(root.path().join("_astro/app.abcdef.js.gz")).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/_astro/app.abcdef.js")
                .header("accept-encoding", "br, gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("content-encoding"));
    assert_eq!(response.headers()["vary"], "accept-encoding");
    assert_eq!(
        to_bytes(response.into_body(), 4096).await.unwrap(),
        "export const okay=true;"
    );
}

#[tokio::test]
async fn html_is_not_precompressed_and_sidecars_are_not_direct_asset_routes() {
    let root = web_fixture();
    std::fs::write(root.path().join("index.html.br"), b"wrong html variant").unwrap();
    std::fs::write(
        root.path().join("_astro/app.abcdef.js"),
        "export const okay=true;",
    )
    .unwrap();
    for suffix in ["br", "gz"] {
        std::fs::write(
            root.path().join(format!("_astro/app.abcdef.js.{suffix}")),
            b"compressed fixture",
        )
        .unwrap();
    }
    let app = routes::<()>(StaticDir::open(root.path()).unwrap());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .header("accept-encoding", "br, gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(!response.headers().contains_key("content-encoding"));
    assert!(to_bytes(response.into_body(), 4096)
        .await
        .unwrap()
        .starts_with(b"<!doctype html>"));
    for path in [
        "/_astro/app.abcdef.js.br",
        "/_astro/app.abcdef.js.gz",
        "/_astro/missing.js",
        "/_astro/%2e%2e/index.html.br",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("accept-encoding", "br, gzip")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert!(!response.headers().contains_key("cache-control"), "{path}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn linked_assets_and_compressed_sidecars_cannot_escape_static_root() {
    let root = web_fixture();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.js");
    std::fs::write(&secret, "PRIVATE-DATA-SENTINEL").unwrap();
    std::fs::write(root.path().join("_astro/app.abcdef.js"), "safe asset").unwrap();
    std::os::unix::fs::symlink(&secret, root.path().join("_astro/escape.js")).unwrap();
    let app = routes::<()>(StaticDir::open(root.path()).unwrap());
    for suffix in ["br", "gz"] {
        let sidecar = root.path().join(format!("_astro/app.abcdef.js.{suffix}"));
        std::os::unix::fs::symlink(&secret, &sidecar).unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/_astro/app.abcdef.js")
                    .header("accept-encoding", suffix.replace("gz", "gzip"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.headers().contains_key("content-encoding"));
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            "safe asset"
        );
        std::fs::remove_file(sidecar).unwrap();
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri("/_astro/escape.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

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
