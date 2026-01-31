use kellnr_appstate::AppStateData;
use kellnr_web_ui::crate_access;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the crate access control (ACL) routes
pub fn create_routes() -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new()
        // Access data (download restrictions)
        .routes(routes!(
            crate_access::get_access_data,
            crate_access::set_access_data
        ))
        // User access
        .routes(routes!(crate_access::list_users))
        .routes(routes!(crate_access::add_user, crate_access::delete_user))
        // Group access
        .routes(routes!(crate_access::list_groups))
        .routes(routes!(crate_access::add_group, crate_access::delete_group))
}
