use appstate::AppStateData;
use auth::auth_req_token;
use axum::{Router, middleware, routing::get};
use index::cratesio_prefetch_api;
use registry::cratesio_api;

/// Creates the crates.io API routes
pub fn create_routes(state: AppStateData) -> Router<AppStateData> {
    Router::new()
        .route("/config.json", get(cratesio_prefetch_api::config_cratesio))
        .route(
            "/{a}/{b}/{name}",
            get(cratesio_prefetch_api::prefetch_cratesio),
        )
        .route(
            "/{a}/{name}",
            get(cratesio_prefetch_api::prefetch_len2_cratesio),
        )
        .route("/", get(cratesio_api::search))
        .route(
            "/dl/{package}/{version}/download",
            get(cratesio_api::download),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            cratesio_api::cratesio_enabled,
        ))
        .route_layer(middleware::from_fn_with_state(
            state,
            auth_req_token::cargo_auth_when_required,
        ))
}
