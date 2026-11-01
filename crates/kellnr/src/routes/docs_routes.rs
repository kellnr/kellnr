use axum::extract::DefaultBodyLimit;
use axum::middleware;
use kellnr_docs::api;
use kellnr_http::identity::Accepts;
use kellnr_http::middleware::auth_when_required;
use kellnr_http::state::AppStateData;
use kellnr_web_ui::ui;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the docs routes
pub(super) fn create_routes(
    state: AppStateData,
    max_docs_bytes: usize,
) -> OpenApiRouter<AppStateData> {
    // Cargo uploads docs with its token, so the upload is not behind the
    // session check of the UI routes. It also needs a custom body limit.
    let upload_router = OpenApiRouter::new()
        .routes(routes!(api::publish_docs))
        .layer(DefaultBodyLimit::max(max_docs_bytes));

    OpenApiRouter::new()
        .routes(routes!(api::docs_in_queue, ui::build_rustdoc))
        .routes(routes!(ui::migrate_docs_storage))
        .routes(routes!(api::latest_docs))
        .layer(middleware::from_fn_with_state(
            (state, Accepts::Session),
            auth_when_required,
        ))
        .merge(upload_router)
}
