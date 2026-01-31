use axum::Router;
use axum::routing::{delete, get, post, put};
use kellnr_appstate::AppStateData;
use kellnr_web_ui::group;

/// Creates the group routes
pub fn create_routes() -> Router<AppStateData> {
    Router::new()
        .route("/", get(group::list_groups))
        .route("/", post(group::add))
        .route("/{name}", delete(group::delete))
        .route("/{group_name}/members", get(group::list_users))
        .route("/{group_name}/members/{name}", put(group::add_user))
        .route("/{group_name}/members/{name}", delete(group::delete_user))
}
