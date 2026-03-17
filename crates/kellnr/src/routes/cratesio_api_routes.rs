use std::sync::Arc;

use axum::routing::get;
use axum::{Router, middleware};
use kellnr_appstate::AppStateData;
use kellnr_auth::auth_req_token;
use kellnr_index::cratesio_prefetch_api;
use kellnr_registry::cratesio_api;
use tokio::sync::Semaphore;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::apply_download_limits;

/// Creates the crates.io API routes
pub fn create_routes(
    state: AppStateData,
    download_semaphore: Option<Arc<Semaphore>>,
) -> OpenApiRouter<AppStateData> {
    let settings = &state.settings.registry;

    // Download route with concurrency limit and timeout to prevent I/O starvation
    let download_router = Router::new().route(
        "/dl/{package}/{version}/download",
        get(cratesio_api::download),
    );

    let download_router = apply_download_limits(download_router, download_semaphore, settings);

    let download_router: OpenApiRouter<AppStateData> = download_router.into();

    OpenApiRouter::new()
        .routes(routes!(cratesio_prefetch_api::config_cratesio))
        .routes(routes!(cratesio_prefetch_api::prefetch_cratesio))
        .routes(routes!(cratesio_prefetch_api::prefetch_len2_cratesio))
        .routes(routes!(cratesio_api::search))
        .merge(download_router)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            cratesio_api::cratesio_enabled,
        ))
        .layer(middleware::from_fn_with_state(
            state,
            auth_req_token::cargo_auth_when_required,
        ))
}
