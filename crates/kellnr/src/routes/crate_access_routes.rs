use axum::Router;
use axum::routing::{delete, get, put};
use kellnr_appstate::AppStateData;
use kellnr_web_ui::crate_access;

/// Creates the crate access control (ACL) routes
pub fn create_routes() -> Router<AppStateData> {
    Router::new()
        // Access data (download restrictions)
        .route("/{crate_name}", get(crate_access::get_access_data))
        .route("/{crate_name}", put(crate_access::set_access_data))
        // User access
        .route("/{crate_name}/users", get(crate_access::list_users))
        .route("/{crate_name}/users/{name}", put(crate_access::add_user))
        .route(
            "/{crate_name}/users/{name}",
            delete(crate_access::delete_user),
        )
        // Group access
        .route("/{crate_name}/groups", get(crate_access::list_groups))
        .route("/{crate_name}/groups/{name}", put(crate_access::add_group))
        .route(
            "/{crate_name}/groups/{name}",
            delete(crate_access::delete_group),
        )
}
