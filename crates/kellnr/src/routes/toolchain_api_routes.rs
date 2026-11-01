use axum::extract::DefaultBodyLimit;
use kellnr_http::state::AppStateData;
use kellnr_toolchain as toolchain;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the toolchain API routes (management endpoints)
pub(super) fn create_api_routes(max_bytes: usize) -> OpenApiRouter<AppStateData> {
    // Upload route needs custom body limit, so it gets its own router
    let upload_router = OpenApiRouter::new()
        .routes(routes!(toolchain::upload_toolchain))
        .layer(DefaultBodyLimit::max(max_bytes));

    OpenApiRouter::new()
        .routes(routes!(toolchain::list_toolchains))
        .merge(upload_router)
        .routes(routes!(toolchain::delete_toolchain))
        .routes(routes!(toolchain::delete_toolchain_target))
        .routes(routes!(toolchain::list_channels))
        .routes(routes!(toolchain::set_channel))
}

/// Creates the toolchain distribution routes (download endpoints)
pub(super) fn create_dist_routes() -> OpenApiRouter<AppStateData> {
    // No authentication on dist routes, rustup does not support HTTP authentication
    OpenApiRouter::new()
        // Use a full segment parameter and parse the manifest filename in the handler
        // because Axum doesn't allow parameters in the middle of a path segment
        .routes(routes!(toolchain::get_channel_manifest))
        .routes(routes!(toolchain::download_archive))
}
