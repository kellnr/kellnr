use kellnr_appstate::AppStateData;
use axum::{
    Router,
    routing::{delete, get, post, put},
};
use kellnr_web_ui::group;

/// Creates the group routes
pub fn create_routes() -> Router<AppStateData> {
    Router::new()
        .route("/", get(group::list_groups))
        .route("/add", post(group::add))
        .route("/delete/{name}", delete(group::delete))
        .route("/{group_name}/users", get(group::list_users))
        .route("/{group_name}/users/{name}", put(group::add_user))
        .route("/{group_name}/users/{name}", delete(group::delete_user))
}
