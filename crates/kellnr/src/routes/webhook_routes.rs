use kellnr_appstate::AppStateData;
use kellnr_webhooks::endpoints;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the webhook routes
pub fn create_routes() -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new()
        .routes(routes!(
            endpoints::get_all_webhooks,
            endpoints::register_webhook
        ))
        .routes(routes!(endpoints::get_webhook, endpoints::delete_webhook))
        .routes(routes!(endpoints::test_webhook))
}
