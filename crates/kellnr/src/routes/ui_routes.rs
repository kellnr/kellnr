use axum::middleware;
use kellnr_appstate::AppStateData;
use kellnr_web_ui::{session, ui};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the UI API routes (JSON endpoints used by the web frontend).
pub fn create_routes(state: AppStateData) -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new()
        .routes(routes!(ui::kellnr_version))
        .routes(routes!(ui::crates))
        .routes(routes!(ui::delete_crate_all))
        .routes(routes!(ui::delete_crate_version))
        .routes(routes!(ui::search))
        .routes(routes!(ui::statistic))
        .routes(routes!(ui::crate_data))
        .routes(routes!(ui::cratesio_data))
        .routes(routes!(ui::settings))
        .layer(middleware::from_fn_with_state(
            state,
            session::session_auth_when_required,
        ))
}
