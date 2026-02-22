use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Router, middleware};
use kellnr_appstate::AppStateData;
use kellnr_auth::auth_req_token;
use kellnr_index::kellnr_prefetch_api;
use kellnr_registry::kellnr_api;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the kellnr API routes
pub fn create_routes(state: AppStateData, max_crate_size: usize) -> OpenApiRouter<AppStateData> {
    let settings = &state.settings.registry;

    // Publish route needs custom body size limit, keep as regular Router
    let publish_router: OpenApiRouter<AppStateData> = Router::new()
        .route(
            "/new",
            put(kellnr_api::publish).layer(DefaultBodyLimit::max(max_crate_size * 1_000_000)),
        )
        .into();

    // Download route with concurrency limit and timeout to prevent I/O starvation
    let mut download_router = Router::new().route(
        "/dl/{package}/{version}/download",
        get(kellnr_api::download),
    );

    if settings.download_max_concurrent > 0 {
        download_router =
            download_router.layer(ConcurrencyLimitLayer::new(settings.download_max_concurrent));
    }
    if settings.download_timeout_seconds > 0 {
        download_router = download_router.layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            Duration::from_secs(settings.download_timeout_seconds),
        ));
    }

    let download_router: OpenApiRouter<AppStateData> = download_router.into();

    OpenApiRouter::new()
        // Prefetch routes
        .routes(routes!(kellnr_prefetch_api::config_kellnr))
        .routes(routes!(kellnr_prefetch_api::prefetch_kellnr))
        .routes(routes!(kellnr_prefetch_api::prefetch_len2_kellnr))
        // Owner routes (multiple methods on same path)
        .routes(routes!(
            kellnr_api::remove_owner,
            kellnr_api::add_owner,
            kellnr_api::list_owners
        ))
        .routes(routes!(
            kellnr_api::remove_owner_single,
            kellnr_api::add_owner_single
        ))
        // Crate user routes
        .routes(routes!(
            kellnr_api::remove_crate_user,
            kellnr_api::add_crate_user
        ))
        .routes(routes!(kellnr_api::list_crate_users))
        // Crate group routes
        .routes(routes!(
            kellnr_api::remove_crate_group,
            kellnr_api::add_crate_group
        ))
        .routes(routes!(kellnr_api::list_crate_groups))
        // Version routes
        .routes(routes!(kellnr_api::list_crate_versions))
        // Search
        .routes(routes!(kellnr_api::search))
        // Download (with concurrency limit and timeout)
        .merge(download_router)
        // Publish routes (merge the custom-layer router)
        .merge(publish_router)
        .routes(routes!(kellnr_api::add_empty_crate))
        // Yank routes
        .routes(routes!(kellnr_api::yank))
        .routes(routes!(kellnr_api::unyank))
        // Accept either cargo token auth (cargo CLI) or session cookie auth (web UI)
        .layer(middleware::from_fn_with_state(
            state,
            auth_req_token::token_or_session_auth_when_required,
        ))
}
