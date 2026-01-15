use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, put};
use axum::{Router, middleware};
use kellnr_appstate::AppStateData;
use kellnr_auth::auth_req_token;
use kellnr_index::kellnr_prefetch_api;
use kellnr_registry::kellnr_api;

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
            "/{crate_name}/owners/{user}",
            delete(kellnr_api::remove_owner_single),
        )
        .route(
            "/{crate_name}/owners/{user}",
            put(kellnr_api::add_owner_single),
        )
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
        .route("/new_empty", put(kellnr_api::add_empty_crate))
        .route("/{crate_name}/{version}/yank", delete(kellnr_api::yank))
        .route("/{crate_name}/{version}/unyank", put(kellnr_api::unyank))
        // Accept either cargo token auth (cargo CLI) or session cookie auth (web UI)
        .route_layer(middleware::from_fn_with_state(
            state,
            auth_req_token::token_or_session_auth_when_required,
        ))
}
