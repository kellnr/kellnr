use kellnr_http::state::AppStateData;
use kellnr_web_ui::crate_access;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the crate access settings routes. Crate users and groups are
/// managed through the registry API under `/api/v1/crates`.
pub fn create_routes() -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new().routes(routes!(
        crate_access::get_access_data,
        crate_access::set_access_data
    ))
}
