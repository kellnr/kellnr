use kellnr_appstate::AppStateData;
use axum::{
    Router, middleware,
    routing::{delete, get},
};
use kellnr_web_ui::{session, ui};

/// Creates the UI API routes (JSON endpoints used by the web frontend).
pub fn create_routes(state: AppStateData) -> Router<AppStateData> {
    Router::new()
        .route("/version", get(ui::kellnr_version))
        .route("/crates", get(ui::crates))
        .route("/search", get(ui::search))
        .route("/statistic", get(ui::statistic))
        .route("/crate_data", get(ui::crate_data))
        .route("/cratesio_data", get(ui::cratesio_data))
        .route("/delete_version", delete(ui::delete_version))
        .route("/delete_crate", delete(ui::delete_crate))
        .route("/settings", get(ui::settings))
        .route_layer(middleware::from_fn_with_state(
            state,
            session::session_auth_when_required,
        ))
}
