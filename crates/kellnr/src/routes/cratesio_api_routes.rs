use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::get;
use kellnr_appstate::AppStateData;
use kellnr_auth::auth_req_token;
use kellnr_index::cratesio_prefetch_api;
use kellnr_registry::cratesio_api;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the crates.io API routes
pub fn create_routes(state: AppStateData) -> OpenApiRouter<AppStateData> {
    let settings = &state.settings.registry;

    // Download route with concurrency limit and timeout to prevent I/O starvation
    let mut download_router = Router::new()
        .route("/dl/{package}/{version}/download", get(cratesio_api::download));

    if settings.download_max_concurrent > 0 {
        download_router = download_router.layer(ConcurrencyLimitLayer::new(settings.download_max_concurrent));
    }
    if settings.download_timeout_seconds > 0 {
        download_router = download_router.layer(TimeoutLayer::with_status_code(StatusCode::GATEWAY_TIMEOUT, Duration::from_secs(settings.download_timeout_seconds)));
    }

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
