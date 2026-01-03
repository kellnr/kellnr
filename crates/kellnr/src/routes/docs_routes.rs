use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post, put};
use axum::{Router, middleware};
use kellnr_appstate::AppStateData;
use kellnr_docs::api;
use kellnr_web_ui::{session, ui};

/// Creates the docs UI routes
pub fn create_ui_routes(state: AppStateData) -> Router<AppStateData> {
    Router::new()
        .route("/build", post(ui::build_rustdoc))
        .route("/queue", get(api::docs_in_queue))
        .route("/{package}/latest", get(api::latest_docs))
        .route_layer(middleware::from_fn_with_state(
            state,
            session::session_auth_when_required,
        ))
}

/// Creates the docs manual routes
pub fn create_manual_routes(max_docs_size: usize) -> Router<AppStateData> {
    Router::new().route(
        "/{package}/{version}",
        put(api::publish_docs).layer(DefaultBodyLimit::max(max_docs_size * 1_000_000)),
    )
}
