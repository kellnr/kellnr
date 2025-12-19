use appstate::AppStateData;
use axum::{
    Router, middleware,
    routing::{get, get_service},
};
use bytes::Bytes;
use include_dir::{Dir, include_dir};
use mime_guess::from_path;
use std::borrow::Cow;
use tower_http::services::ServeDir;
use tracing::{debug, warn};
use web_ui::session;

use axum::{
    body::Body,
    http::{Response, StatusCode, Uri, header},
};

mod crate_access_routes;
mod cratesio_api_routes;
mod docs_routes;
mod group_routes;
mod health_routes;
mod kellnr_api_routes;
mod ui_routes;
mod user_routes;

// Embedded UI assets (copied from `ui/dist` into workspace `static/` by `just npm-build`).
// NOTE: This file lives in `crates/kellnr/src/routes/`, so we need to go up to the workspace root.
static STATIC_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../static");

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

        debug!(path = normalized, "serving embedded ui asset");

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

async fn embedded_static_handler(uri: Uri) -> Response<Body> {
    // `Uri::path()` always starts with `/`
    let path = uri.path();

    debug!(path, "ui request");

    // Map `/` to `index.html`
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

/// Creates and returns the complete application router with all routes configured
pub fn create_router(
    state: AppStateData,
    data_dir: &str,
    max_docs_size: usize,
    max_crate_size: usize,
) -> Router {
    // Docs are served from disk (generated/managed at runtime)
    let docs_service = get_service(ServeDir::new(format!("{data_dir}/docs"))).route_layer(
        middleware::from_fn_with_state(state.clone(), session::session_auth_when_required),
    );

    // Combine all routes into the main application router
    Router::new()
        .route("/me", get(registry::kellnr_api::me))
        .nest("/api/v1/ui", ui_routes::create_routes(state.clone()))
        .nest("/api/v1/user", user_routes::create_routes())
        .nest("/api/v1/group", group_routes::create_routes())
        .nest("/api/v1/crate_access", crate_access_routes::create_routes())
        .nest("/api/v1/docs", docs_routes::create_ui_routes(state.clone()))
        .nest(
            "/api/v1/docs",
            docs_routes::create_manual_routes(max_docs_size),
        )
        .nest(
            "/api/v1/crates",
            kellnr_api_routes::create_routes(state.clone(), max_crate_size),
        )
        .nest(
            "/api/v1/cratesio",
            cratesio_api_routes::create_routes(state.clone()),
        )
        .nest("/api/v1", health_routes::create_routes())
        .nest_service("/docs", docs_service)
        // Always serve the UI from the embedded directory (single-binary deploy).
        .fallback(get(embedded_static_handler))
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http())
}
