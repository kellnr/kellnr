use std::sync::Arc;

use axum::routing::{get, get_service};
use axum::{Extension, Router, middleware};
use kellnr_appstate::AppStateData;
use kellnr_auth::oauth2::OAuth2Handler;
use kellnr_embedded_resources::embedded_static_handler;
use kellnr_web_ui::session;
use tower_http::services::ServeDir;

mod crate_access_routes;
mod cratesio_api_routes;
mod docs_routes;
mod group_routes;
mod health_routes;
mod kellnr_api_routes;
mod oauth2_routes;
mod toolchain_routes;
mod ui_routes;
mod user_routes;
mod webhook_routes;

pub fn create_router(
    state: AppStateData,
    data_dir: &str,
    max_docs_size: usize,
    max_crate_size: usize,
    max_toolchain_size: usize,
    oauth2_handler: Option<Arc<OAuth2Handler>>,
) -> Router {
    // Docs are served from disk and not from embedded assets
    let docs_service = get_service(ServeDir::new(format!("{data_dir}/docs"))).route_layer(
        middleware::from_fn_with_state(state.clone(), session::session_auth_when_required),
    );

    let mut router = Router::new()
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
        .nest("/api/v1/webhook", webhook_routes::create_routes())
        .nest("/api/v1/oauth2", oauth2_routes::create_routes())
        .nest("/api/v1", health_routes::create_routes())
        .nest_service("/docs", docs_service);

    // Conditionally add toolchain routes if enabled
    if state.settings.toolchain.enabled {
        router = router
            .nest(
                "/api/v1/toolchain",
                toolchain_routes::create_api_routes(state.clone(), max_toolchain_size),
            )
            .nest(
                "/api/v1/toolchain/dist",
                toolchain_routes::create_dist_routes(state.clone()),
            );
    }

    router
        // Always serve the UI from the embedded directory (single-binary deploy).
        .fallback(get(embedded_static_handler))
        .with_state(state)
        // Add OAuth2 handler as an extension (accessible via Extension<Option<Arc<OAuth2Handler>>>)
        .layer(Extension(oauth2_handler))
        .layer(tower_http::trace::TraceLayer::new_for_http())
}
