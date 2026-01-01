use axum::{
    body::Body,
    http::{Response, StatusCode, Uri, header},
};
use bytes::Bytes;
use include_dir::{Dir, include_dir};
use mime_guess::from_path;
use std::borrow::Cow;
use tracing::warn;

static STATIC_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/static");

fn cache_control_for_path(path: &str) -> &'static str {
    // For SPAs, avoid caching `index.html` aggressively so deploys update quickly.
    if path.ends_with("index.html") || path == "index.html" {
        "no-cache"
    } else {
        // Most assets are fingerprinted by Vite (e.g. `index-<hash>.js`), so we can cache hard.
        "public, max-age=31536000, immutable"
    }
}

fn serve_embedded_asset(path: &str) -> Response<Body> {
    let normalized = path.trim_start_matches('/');

    if let Some(file) = STATIC_DIR.get_file(normalized) {
        let mime = from_path(normalized).first_or_octet_stream();
        let body = Body::from(Bytes::copy_from_slice(file.contents()));

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, cache_control_for_path(normalized))
            .body(body)
            .unwrap()
    } else {
        warn!(path = normalized, "embedded ui asset not found");
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(Cow::Borrowed("404 Not Found")))
            .unwrap()
    }
}

// Handler has to be async to fit into axum routing, even though we don't do any async work here.
#[allow(clippy::unused_async)]
pub async fn embedded_static_handler(uri: Uri) -> Response<Body> {
    let path = uri.path();

    if path == "/" {
        return serve_embedded_asset("index.html");
    }

    // Try serving embedded file if it exists
    let candidate = path.trim_start_matches('/');
    if STATIC_DIR.get_file(candidate).is_some() {
        return serve_embedded_asset(candidate);
    }

    // SPA fallback: for any non-asset route, serve `index.html`
    // (but keep true 404s for unknown files under /assets or /img etc.)
    let looks_like_asset = candidate.contains('.')
        || candidate.starts_with("assets/")
        || candidate.starts_with("img/");
    if looks_like_asset {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(Cow::Borrowed("404 Not Found")))
            .unwrap();
    }

    serve_embedded_asset("index.html")
}
