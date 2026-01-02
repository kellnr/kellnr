use kellnr_appstate::AppStateData;
use axum::{
    Router,
    routing::{delete, get, post},
};

/// Creates the webhook routes
pub fn create_routes() -> Router<AppStateData> {
    Router::new()
        .route("/", get(kellnr_webhooks::get_all_webhooks))
        .route("/", post(kellnr_webhooks::register_webhook))
        .route("/{id}", get(kellnr_webhooks::get_webhook))
        .route("/{id}", delete(kellnr_webhooks::delete_webhook))
        .route("/{id}/test", get(kellnr_webhooks::test_webhook))
}
