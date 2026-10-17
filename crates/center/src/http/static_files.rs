use axum::{
    extract::{Path, Request},
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::{
    path::{Path as FilePath, PathBuf},
    sync::Arc,
};
use tower::ServiceExt;
use tower_http::services::ServeFile;
const HTML_CACHE: &str = "no-store";
const ASSET_CACHE: &str = "public, max-age=31536000, immutable";
const MAX_ASSET_PATH_BYTES: usize = 512;
const PAGES: [(&str, &str); 7] = [
    ("/", "index.html"),
    ("/login", "login/index.html"),
    ("/login/", "login/index.html"),
    ("/history", "history/index.html"),
    ("/history/", "history/index.html"),
    ("/settings", "settings/index.html"),
    ("/settings/", "settings/index.html"),
];
#[derive(Clone)]
pub struct StaticDir(Arc<FilePath>);
#[derive(Debug, thiserror::Error)]
#[error("static directory must contain the complete web build")]
pub struct StaticError;
impl StaticDir {
    pub fn open(directory: &FilePath) -> Result<Self, StaticError> {
        if !directory.is_absolute() || !directory.symlink_metadata().is_ok_and(|meta| meta.is_dir())
        {
            return Err(StaticError);
        }
        let root = directory.canonicalize().map_err(|_| StaticError)?;
        for (_, file) in PAGES {
            let path = root.join(file);
            if !path.symlink_metadata().is_ok_and(|meta| meta.is_file())
                || !path
                    .canonicalize()
                    .map_err(|_| StaticError)?
                    .starts_with(&root)
            {
                return Err(StaticError);
            }
        }
        Ok(Self(root.into()))
    }
}
async fn headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    response
        .headers_mut()
        .insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response
}
fn cache(mut response: Response, value: &'static str) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    response
}
async fn asset(root: StaticDir, name: String, request: Request) -> Response {
    if name.len() > MAX_ASSET_PATH_BYTES
        || name.split('/').any(|part| {
            part.is_empty()
                || part.starts_with('.')
                || part
                    .chars()
                    .any(|ch| ch.is_control() || matches!(ch, '\\' | ':' | '%'))
        })
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let asset_root = root.0.join("_astro");
    let canonical = match asset_root.join(name).canonicalize() {
        Ok(value)
            if value.starts_with(root.0.as_ref())
                && value.starts_with(&asset_root)
                && value.is_file() =>
        {
            value
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    match ServeFile::new(canonical).oneshot(request).await {
        Ok(response) => cache(response.into_response(), ASSET_CACHE),
        Err(error) => match error {},
    }
}
pub fn routes<S: Clone + Send + Sync + 'static>(directory: StaticDir) -> Router<S> {
    let mut router = Router::new();
    for (route, file) in PAGES {
        let path: PathBuf = directory.0.join(file);
        router = router.route(
            route,
            get(move |request: Request| {
                let path = path.clone();
                async move {
                    match ServeFile::new(path).oneshot(request).await {
                        Ok(response) => cache(response.into_response(), HTML_CACHE),
                        Err(error) => match error {},
                    }
                }
            }),
        );
    }
    router
        .route(
            "/_astro/{*path}",
            get(move |Path(path): Path<String>, request: Request| {
                asset(directory.clone(), path, request)
            }),
        )
        .layer(middleware::from_fn(headers))
}
