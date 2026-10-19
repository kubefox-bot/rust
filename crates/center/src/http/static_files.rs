mod page;
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
pub(crate) fn is_page(path: &str) -> bool {
    PAGES.iter().any(|(route, _)| *route == path)
}
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
        || name.ends_with(".br")
        || name.ends_with(".gz")
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
    let service = ServeFile::new(&canonical);
    let service = if has_safe_sidecar(&asset_root, &canonical, ".br") {
        service.precompressed_br()
    } else {
        service
    };
    let service = if has_safe_sidecar(&asset_root, &canonical, ".gz") {
        service.precompressed_gzip()
    } else {
        service
    };
    match service.oneshot(request).await {
        Ok(response) => {
            let mut response = response.into_response();
            response
                .headers_mut()
                .insert(header::VARY, HeaderValue::from_static("accept-encoding"));
            if response.status().is_success() || response.status() == StatusCode::NOT_MODIFIED {
                cache(response, ASSET_CACHE)
            } else {
                response
            }
        }
        Err(error) => match error {},
    }
}
fn has_safe_sidecar(root: &FilePath, path: &FilePath, suffix: &str) -> bool {
    let mut sidecar = path.as_os_str().to_os_string();
    sidecar.push(suffix);
    FilePath::new(&sidecar)
        .canonicalize()
        .is_ok_and(|path| path.starts_with(root) && path.is_file())
}
pub fn routes<S: Clone + Send + Sync + 'static>(directory: StaticDir) -> Router<S> {
    let mut router = Router::new();
    for (route, file) in PAGES {
        let path: PathBuf = directory.0.join(file);
        router = router.route(
            route,
            get(move |request: Request| {
                let path = path.clone();
                async move { page::serve(&path, request).await }
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
