use axum::middleware;
use kellnr_appstate::AppStateData;
use kellnr_auth::auth_req_token;
use kellnr_index::cratesio_prefetch_api;
use kellnr_registry::cratesio_api;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the crates.io API routes
pub fn create_routes(state: AppStateData) -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new()
        .routes(routes!(cratesio_prefetch_api::config_cratesio))
        .routes(routes!(cratesio_prefetch_api::prefetch_cratesio))
        .routes(routes!(cratesio_prefetch_api::prefetch_len2_cratesio))
        .routes(routes!(cratesio_api::search))
        .routes(routes!(cratesio_api::download))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            cratesio_api::cratesio_enabled,
        ))
        .layer(middleware::from_fn_with_state(
            state,
            auth_req_token::cargo_auth_when_required,
        ))
}
