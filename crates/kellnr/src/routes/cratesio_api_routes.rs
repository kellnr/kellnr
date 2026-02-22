use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::middleware;
use axum::middleware::Next;
use axum::routing::get;
use kellnr_appstate::AppStateData;
use kellnr_auth::auth_req_token;
use kellnr_index::cratesio_prefetch_api;
use kellnr_registry::cratesio_api;
use tokio::sync::Semaphore;
use tower_http::timeout::TimeoutLayer;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the crates.io API routes
pub fn create_routes(
    state: AppStateData,
    download_semaphore: Option<Arc<Semaphore>>,
) -> OpenApiRouter<AppStateData> {
    let settings = &state.settings.registry;

    // Download route with concurrency limit and timeout to prevent I/O starvation
    let mut download_router = Router::new().route(
        "/dl/{package}/{version}/download",
        get(cratesio_api::download),
    );

    if let Some(semaphore) = download_semaphore {
        download_router = download_router.layer(middleware::from_fn(
            move |req: axum::extract::Request, next: Next| {
                let sem = semaphore.clone();
                async move {
                    let _permit = sem.acquire().await.expect("download semaphore closed");
                    next.run(req).await
                }
            },
        ));
    }
    if settings.download_timeout_seconds > 0 {
        let timeout_secs = settings.download_timeout_seconds;
        download_router = download_router
            .layer(middleware::map_response(
                move |response: axum::response::Response| async move {
                    if response.status() == StatusCode::GATEWAY_TIMEOUT {
                        tracing::warn!(
                            "Download request timed out after {timeout_secs}s. \
                             Consider increasing registry.download_timeout_seconds"
                        );
                    }
                    response
                },
            ))
            .layer(TimeoutLayer::with_status_code(
                StatusCode::GATEWAY_TIMEOUT,
                Duration::from_secs(timeout_secs),
            ));
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
