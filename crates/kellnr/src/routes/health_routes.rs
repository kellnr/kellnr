use appstate::AppStateData;
use axum::{Router, routing::get};

/// Health check route
pub fn create_routes() -> Router<AppStateData> {
    Router::new().route("/health", get(health_check))
}

pub async fn health_check() -> &'static str {
    "OK"
}
