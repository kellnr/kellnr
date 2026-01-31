use kellnr_appstate::AppStateData;
use kellnr_web_ui::group;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the group routes
pub fn create_routes() -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new()
        .routes(routes!(group::list_groups, group::add))
        .routes(routes!(group::delete))
        .routes(routes!(group::list_users))
        .routes(routes!(group::add_user, group::delete_user))
}
