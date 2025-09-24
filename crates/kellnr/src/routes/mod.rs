use appstate::AppStateData;
use axum::{
    Router, middleware,
    routing::{get, get_service},
};
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};
use web_ui::session;

mod crate_access_routes;
mod cratesio_api_routes;
mod docs_routes;
mod group_routes;
mod health_routes;
mod kellnr_api_routes;
mod ui_routes;
mod user_routes;

/// Creates and returns the complete application router with all routes configured
pub fn create_router(
    state: AppStateData,
    data_dir: &str,
    max_docs_size: usize,
    max_crate_size: usize,
) -> Router {
    // Setup static files service
    let static_path = Path::new(option_env!("KELLNR_STATIC_DIR").unwrap_or("./static"));
    let static_files_service = get_service(
        ServeDir::new(static_path)
            .append_index_html_on_directories(true)
            .fallback(ServeFile::new(static_path.join("index.html"))),
    );

    // Setup docs service
    let docs_service = get_service(ServeDir::new(format!("{data_dir}/docs"))).route_layer(
        middleware::from_fn_with_state(state.clone(), session::session_auth_when_required),
    );
    let origin_path = state.settings.origin.path.clone();

    // Combine all routes into the main application router
    let router = Router::new()
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
        .fallback(static_files_service)
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    if origin_path.is_empty() || origin_path.chars().all(|c| c == '/') {
        router
    } else if !origin_path.starts_with('/') || !origin_path.ends_with('/') {
        eprintln!(
            "origin.path needs to start and end with a slash(/): origin.path={origin_path:?}"
        );
        std::process::exit(1);
    } else {
        Router::new().nest(&origin_path, router)
    }
}
