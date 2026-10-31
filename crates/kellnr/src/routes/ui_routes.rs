use axum::middleware;
use kellnr_http::identity::Accepts;
use kellnr_http::middleware::auth_when_required;
use kellnr_http::state::AppStateData;
use kellnr_web_ui::ui;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the UI API routes (JSON endpoints used by the web frontend).
pub(super) fn create_routes(state: AppStateData) -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new()
        .routes(routes!(ui::kellnr_version))
        .routes(routes!(ui::crates))
        .routes(routes!(ui::delete_crate_all))
        .routes(routes!(ui::delete_crate_version))
        .routes(routes!(ui::search))
        .routes(routes!(ui::statistic))
        .routes(routes!(ui::crate_data))
        .routes(routes!(ui::crate_dependents))
        .routes(routes!(ui::cratesio_data))
        .routes(routes!(ui::settings))
        .routes(routes!(ui::docs_enabled))
        .routes(routes!(ui::sponsor_links_enabled))
        .layer(middleware::from_fn_with_state(
            (state, Accepts::Session),
            auth_when_required,
        ))
}
