use axum::Router;
use axum::routing::get;
use kellnr_appstate::AppStateData;

/// Health check route
pub fn create_routes() -> Router<AppStateData> {
    Router::new().route("/health", get(health_check))
}

pub async fn health_check() -> &'static str {
    "OK"
}
