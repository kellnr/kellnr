use appstate::AppStateData;
use auth::auth_req_token;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{delete, get, put},
};
use index::kellnr_prefetch_api;
use registry::kellnr_api;

/// Creates the kellnr API routes
pub fn create_routes(state: AppStateData, max_crate_size: usize) -> Router<AppStateData> {
    Router::new()
        .route("/config.json", get(kellnr_prefetch_api::config_kellnr))
        .route(
            "/{a}/{b}/{package}",
            get(kellnr_prefetch_api::prefetch_kellnr),
        )
        .route(
            "/{a}/{package}",
            get(kellnr_prefetch_api::prefetch_len2_kellnr),
        )
        .route("/{crate_name}/owners", delete(kellnr_api::remove_owner))
        .route("/{crate_name}/owners", put(kellnr_api::add_owner))
        .route("/{crate_name}/owners", get(kellnr_api::list_owners))
        .route(
            "/{crate_name}/crate_users/{user}",
            delete(kellnr_api::remove_crate_user),
        )
        .route(
            "/{crate_name}/crate_users/{user}",
            put(kellnr_api::add_crate_user),
        )
        .route(
            "/{crate_name}/crate_users",
            get(kellnr_api::list_crate_users),
        )
        .route(
            "/{crate_name}/crate_groups/{group}",
            delete(kellnr_api::remove_crate_group),
        )
        .route(
            "/{crate_name}/crate_groups/{group}",
            put(kellnr_api::add_crate_group),
        )
        .route(
            "/{crate_name}/crate_groups",
            get(kellnr_api::list_crate_groups),
        )
        .route(
            "/{crate_name}/crate_versions",
            get(kellnr_api::list_crate_versions),
        )
        .route("/", get(kellnr_api::search))
        .route(
            "/dl/{package}/{version}/download",
            get(kellnr_api::download),
        )
        .route(
            "/new",
            put(kellnr_api::publish).layer(DefaultBodyLimit::max(max_crate_size * 1_000_000)),
        )
        .route("/{crate_name}/{version}/yank", delete(kellnr_api::yank))
        .route("/{crate_name}/{version}/unyank", put(kellnr_api::unyank))
        .route_layer(middleware::from_fn_with_state(
            state,
            auth_req_token::cargo_auth_when_required,
        ))
}
