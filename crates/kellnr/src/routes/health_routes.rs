use kellnr_appstate::AppStateData;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Health check route
pub fn create_routes() -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new().routes(routes!(health_check))
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service healthy", body = String)
    )
)]
pub async fn health_check() -> &'static str {
    "OK"
}
