use axum::Router;
use axum::routing::{delete, get, post};
use kellnr_appstate::AppStateData;

/// Creates the webhook routes
pub fn create_routes() -> Router<AppStateData> {
    Router::new()
        .route("/", get(kellnr_webhooks::get_all_webhooks))
        .route("/", post(kellnr_webhooks::register_webhook))
        .route("/{id}", get(kellnr_webhooks::get_webhook))
        .route("/{id}", delete(kellnr_webhooks::delete_webhook))
        .route("/{id}/test", post(kellnr_webhooks::test_webhook))
}
