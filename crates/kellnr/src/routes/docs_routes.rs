use axum::extract::DefaultBodyLimit;
use axum::routing::put;
use axum::{Router, middleware};
use kellnr_appstate::AppStateData;
use kellnr_docs::api;
use kellnr_web_ui::{session, ui};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the docs UI routes
pub fn create_ui_routes(state: AppStateData) -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new()
        .routes(routes!(api::docs_in_queue, ui::build_rustdoc))
        .routes(routes!(api::latest_docs))
        .layer(middleware::from_fn_with_state(
            state,
            session::session_auth_when_required,
        ))
}

/// Creates the docs manual routes
pub fn create_manual_routes(max_docs_size: usize) -> Router<AppStateData> {
    // Keep as regular Router due to custom layer on specific route
    Router::new().route(
        "/{package}/{version}",
        put(api::publish_docs).layer(DefaultBodyLimit::max(max_docs_size * 1_000_000)),
    )
}
