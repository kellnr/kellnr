use appstate::AppStateData;
use axum::{
    routing::{delete, get, post},
    Router,
};

/// Creates the webhook routes
pub fn create_routes() -> Router<AppStateData> {
    Router::new()
        .route("/", get(webhooks::get_all_webhooks))
        .route("/", post(webhooks::register_webhook))
        .route("/{id}", get(webhooks::get_webhook))
        .route("/{id}", delete(webhooks::delete_webhook))
        .route("/{id}/test", get(webhooks::test_webhook))
}
