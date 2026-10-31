use std::convert::TryFrom;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use chrono::Utc;
use kellnr_appstate::{AppState, DbState};
pub use kellnr_auth::crate_access::has_restricted_crate_access;
use kellnr_auth::crate_access::{can_read_crate, crate_visibility};
use kellnr_auth::{maybe_user, token};
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::original_name::OriginalName;
use kellnr_common::search_result;
use kellnr_common::search_result::{Crate, SearchResult};
use kellnr_common::version::Version;
use kellnr_common::webhook::WebhookEvent;
use kellnr_db::DbProvider;
use kellnr_db::error::DbError;
use kellnr_db::provider::DbResult;
use kellnr_error::api_error::{ApiError, ApiResult};
use kellnr_filter::CrateInfo;
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
use kellnr_storage::storage_error::StorageError;
use tracing::{error, warn};

use crate::pub_data::{EmptyCrateData, PubData};
use crate::pub_success::{EmptyCrateSuccess, PubDataSuccess};
use crate::registry_error::RegistryError;
use crate::search_params::SearchParams;
use crate::yank_success::YankSuccess;
use crate::{crate_group, crate_user, crate_version};

pub async fn check_ownership(
    crate_name: &CanonicalName,
    user: &maybe_user::MaybeUser,
    db: &Arc<dyn DbProvider>,
) -> Result<(), ApiError> {
    if user.is_admin || db.is_owner(crate_name, &user.name).await? {
        Ok(())
    } else {
        Err(RegistryError::NotOwner.into())
    }
}

/// Fail like the database does if `name` is spelled with other separators
/// than the existing crate `existing` of the same canonical name. Checked
/// before the ownership, so that everyone gets the same error.
fn ensure_same_separators(name: &OriginalName, existing: &OriginalName) -> ApiResult<()> {
    if name.to_lowercase() == existing.to_lowercase() {
        Ok(())
    } else {
        Err(DbError::CrateNameConflict(name.to_lowercase(), existing.to_string()).into())
    }
}

/// Find an existing version of the crate that equals `version` when build
/// metadata is ignored.
async fn find_same_version(
    crate_name: &CanonicalName,
    version: &Version,
    db: &Arc<dyn DbProvider>,
) -> DbResult<Option<String>> {
    Ok(db
        .get_crate_meta_list(crate_name)
        .await?
        .into_iter()
        .map(|meta| meta.version)
        .find(|existing| {
            Version::try_from(existing.as_str()).is_ok_and(|v| v.eq_ignoring_build(version))
        }))
}

pub fn check_can_modify(user: &maybe_user::MaybeUser) -> Result<(), ApiError> {
    if !user.is_admin && user.is_read_only {
        Err(RegistryError::ReadOnlyModify.into())
    } else {
        Ok(())
    }
}

/// Fail with "crate not found" unless the crate exists and `user` may see it.
/// A download-restricted crate gets the same response as a missing one, so
/// that its existence is not revealed.
async fn check_crate_visible(
    crate_name: &CanonicalName,
    user: Option<&maybe_user::MaybeUser>,
    db: &Arc<dyn DbProvider>,
) -> ApiResult<()> {
    if db.get_crate_id(crate_name).await?.is_some() && can_read_crate(crate_name, user, db).await? {
        Ok(())
    } else {
        Err(RegistryError::CrateNotFound.into())
    }
}

/// Fail with "crate not found" unless the caller may download the crate. A
/// download-restricted crate gets the same response as a missing one, so that
/// its existence is not revealed.
pub async fn check_download_auth(
    crate_name: &CanonicalName,
    token: &token::OptionToken,
    db: &Arc<dyn DbProvider>,
) -> ApiResult<()> {
    if !db.is_download_restricted(crate_name).await? {
        return Ok(());
    }

    let has_access = match token {
        token::OptionToken::Some(token) => {
            has_restricted_crate_access(crate_name, &token.user, token.is_admin, db).await?
        }
        token::OptionToken::None => false,
    };

    if has_access {
        Ok(())
    } else {
        Err(RegistryError::CrateNotFound.into())
    }
}

/// Remove owners from a crate
///
/// Removes one or more owners from a crate. Cannot remove the last owner
/// unless `allow_ownerless_crates` is enabled.
#[utoipa::path(
    delete,
    path = "/{crate_name}/owners",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name")
    ),
    request_body = crate_user::CrateUserRequest,
    responses(
        (status = 200, description = "Owners removed", body = crate_user::CrateUserResponse),
        (status = 403, description = "Not an owner"),
        (status = 404, description = "User is not an owner"),
        (status = 409, description = "Cannot remove last owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn remove_owner(
    user: maybe_user::MaybeUser,
    state: AppState,
    Path(crate_name): Path<OriginalName>,
    Json(input): Json<crate_user::CrateUserRequest>,
) -> ApiResult<Json<crate_user::CrateUserResponse>> {
    // Check if user is read-only and can't remove an owner.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&user)?;

    let settings = &state.settings;
    let db = &state.db;

    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, db).await?;

    db.delete_owners(
        &crate_name,
        &input.users,
        settings.registry.allow_ownerless_crates,
    )
    .await?;

    Ok(Json(crate_user::CrateUserResponse::from(
        "Removed owners from crate.",
    )))
}

/// Remove a single owner from a crate
///
/// Removes a single owner from a crate by username.
#[utoipa::path(
    delete,
    path = "/{crate_name}/owners/{user}",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name"),
        ("user" = String, Path, description = "Username to remove")
    ),
    responses(
        (status = 200, description = "Owner removed", body = crate_user::CrateUserResponse),
        (status = 403, description = "Not an owner"),
        (status = 404, description = "User is not an owner"),
        (status = 409, description = "Cannot remove last owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn remove_owner_single(
    user: maybe_user::MaybeUser,
    state: AppState,
    Path((crate_name, removed_user)): Path<(OriginalName, String)>,
) -> ApiResult<Json<crate_user::CrateUserResponse>> {
    check_can_modify(&user)?;

    let settings = &state.settings;
    let db = &state.db;

    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, db).await?;

    db.delete_owners(
        &crate_name,
        &[removed_user],
        settings.registry.allow_ownerless_crates,
    )
    .await?;

    Ok(Json(crate_user::CrateUserResponse::from(
        "Removed owner from crate.",
    )))
}

/// Remove a user from crate access list
///
/// Removes a user's access to a restricted crate.
#[utoipa::path(
    delete,
    path = "/{crate_name}/crate_users/{user}",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name"),
        ("user" = String, Path, description = "Username to remove")
    ),
    responses(
        (status = 200, description = "User removed", body = crate_user::CrateUserResponse),
        (status = 403, description = "Not an owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn remove_crate_user(
    user: maybe_user::MaybeUser,
    State(db): DbState,
    Path((crate_name, name)): Path<(OriginalName, String)>,
) -> ApiResult<Json<crate_user::CrateUserResponse>> {
    // Check if user is read-only and can't remove a crate user.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&user)?;

    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, &db).await?;

    db.delete_crate_user(&crate_name, &name).await?;
    Ok(Json(crate_user::CrateUserResponse::from(
        "Removed users from crate.",
    )))
}

/// Remove a group from crate access list
///
/// Removes a group's access to a restricted crate.
#[utoipa::path(
    delete,
    path = "/{crate_name}/crate_groups/{group}",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name"),
        ("group" = String, Path, description = "Group name to remove")
    ),
    responses(
        (status = 200, description = "Group removed", body = crate_group::CrateGroupResponse),
        (status = 403, description = "Not an owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn remove_crate_group(
    user: maybe_user::MaybeUser,
    State(db): DbState,
    Path((crate_name, name)): Path<(OriginalName, String)>,
) -> ApiResult<Json<crate_group::CrateGroupResponse>> {
    // Check if user is read-only and can't remove a crate group.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&user)?;

    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, &db).await?;

    db.delete_crate_group(&crate_name, &name).await?;
    Ok(Json(crate_group::CrateGroupResponse::from(
        "Removed groups from crate.",
    )))
}

/// Add owners to a crate
///
/// Adds one or more owners to a crate.
#[utoipa::path(
    put,
    path = "/{crate_name}/owners",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name")
    ),
    request_body = crate_user::CrateUserRequest,
    responses(
        (status = 200, description = "Owners added", body = crate_user::CrateUserResponse),
        (status = 403, description = "Not an owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn add_owner(
    user: maybe_user::MaybeUser,
    State(db): DbState,
    Path(crate_name): Path<OriginalName>,
    Json(input): Json<crate_user::CrateUserRequest>,
) -> ApiResult<Json<crate_user::CrateUserResponse>> {
    // Check if user is read-only and can't add owners.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&user)?;

    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, &db).await?;

    for user in &input.users {
        db.add_owner(&crate_name, user).await?;
    }

    Ok(Json(crate_user::CrateUserResponse::from(
        "Added owners to crate.",
    )))
}

/// Add a single owner to a crate
///
/// Adds a single owner to a crate by username.
#[utoipa::path(
    put,
    path = "/{crate_name}/owners/{user}",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name"),
        ("user" = String, Path, description = "Username to add")
    ),
    responses(
        (status = 200, description = "Owner added", body = crate_user::CrateUserResponse),
        (status = 403, description = "Not an owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn add_owner_single(
    user: maybe_user::MaybeUser,
    State(db): DbState,
    Path((crate_name, added_user)): Path<(OriginalName, String)>,
) -> ApiResult<Json<crate_user::CrateUserResponse>> {
    check_can_modify(&user)?;

    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, &db).await?;

    db.add_owner(&crate_name, &added_user).await?;

    Ok(Json(crate_user::CrateUserResponse::from(
        "Added owner to crate.",
    )))
}

/// List crate owners
///
/// Returns the list of owners for a crate.
#[utoipa::path(
    get,
    path = "/{crate_name}/owners",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name")
    ),
    responses(
        (status = 200, description = "List of owners", body = crate_user::CrateUserList),
        (status = 404, description = "Crate not found or download-restricted without access")
    ),
    security(("cargo_token" = []))
)]
pub async fn list_owners(
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
    user: Result<maybe_user::MaybeUser, StatusCode>,
) -> ApiResult<Json<crate_user::CrateUserList>> {
    let crate_name = crate_name.to_canonical();
    check_crate_visible(&crate_name, user.ok().as_ref(), &db).await?;

    let owners: Vec<crate_user::CrateUser> = db
        .get_crate_owners(&crate_name)
        .await?
        .iter()
        .map(|u| crate_user::CrateUser {
            id: u.id,
            login: u.name.clone(),
            name: None,
        })
        .collect();

    Ok(Json(crate_user::CrateUserList::from(owners)))
}

/// Add a user to crate access list
///
/// Adds a user to the access list of a restricted crate.
#[utoipa::path(
    put,
    path = "/{crate_name}/crate_users/{user}",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name"),
        ("user" = String, Path, description = "Username to add")
    ),
    responses(
        (status = 200, description = "User added", body = crate_user::CrateUserResponse),
        (status = 403, description = "Not an owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn add_crate_user(
    user: maybe_user::MaybeUser,
    State(db): DbState,
    Path((crate_name, name)): Path<(OriginalName, String)>,
) -> ApiResult<Json<crate_user::CrateUserResponse>> {
    // Check if user is read-only and can't add a crate user.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&user)?;

    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, &db).await?;

    if !db.is_crate_user(&crate_name, &name).await? {
        db.add_crate_user(&crate_name, &name).await?;
    }

    Ok(Json(crate_user::CrateUserResponse::from(
        "Added users to crate.",
    )))
}

/// List crate users
///
/// Returns the list of users with access to a restricted crate.
#[utoipa::path(
    get,
    path = "/{crate_name}/crate_users",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name")
    ),
    responses(
        (status = 200, description = "List of users", body = crate_user::CrateUserList),
        (status = 403, description = "Not an owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn list_crate_users(
    user: maybe_user::MaybeUser,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> ApiResult<Json<crate_user::CrateUserList>> {
    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, &db).await?;

    let users: Vec<crate_user::CrateUser> = db
        .get_crate_users(&crate_name)
        .await?
        .iter()
        .map(|u| crate_user::CrateUser {
            id: u.id,
            login: u.name.clone(),
            name: None,
        })
        .collect();

    Ok(Json(crate_user::CrateUserList::from(users)))
}

/// Add a group to crate access list
///
/// Adds a group to the access list of a restricted crate.
#[utoipa::path(
    put,
    path = "/{crate_name}/crate_groups/{group}",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name"),
        ("group" = String, Path, description = "Group name to add")
    ),
    responses(
        (status = 200, description = "Group added", body = crate_group::CrateGroupResponse),
        (status = 403, description = "Not an owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn add_crate_group(
    user: maybe_user::MaybeUser,
    State(db): DbState,
    Path((crate_name, name)): Path<(OriginalName, String)>,
) -> ApiResult<Json<crate_group::CrateGroupResponse>> {
    // Check if user is read-only and can't add a crate group.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&user)?;

    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, &db).await?;

    if !db.is_crate_group(&crate_name, &name).await? {
        db.add_crate_group(&crate_name, &name).await?;
    }

    Ok(Json(crate_group::CrateGroupResponse::from(
        "Added groups to crate.",
    )))
}

/// List crate groups
///
/// Returns the list of groups with access to a restricted crate.
#[utoipa::path(
    get,
    path = "/{crate_name}/crate_groups",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name")
    ),
    responses(
        (status = 200, description = "List of groups", body = crate_group::CrateGroupList),
        (status = 403, description = "Not an owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn list_crate_groups(
    user: maybe_user::MaybeUser,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> ApiResult<Json<crate_group::CrateGroupList>> {
    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, &db).await?;

    let groups: Vec<crate_group::CrateGroup> = db
        .get_crate_groups(&crate_name)
        .await?
        .iter()
        .map(|u| crate_group::CrateGroup {
            id: u.id,
            name: u.name.clone(),
        })
        .collect();

    Ok(Json(crate_group::CrateGroupList::from(groups)))
}

/// List crate versions
///
/// Returns all available versions of a crate.
#[utoipa::path(
    get,
    path = "/{crate_name}/crate_versions",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name")
    ),
    responses(
        (status = 200, description = "List of versions", body = crate_version::CrateVersionList),
        (status = 404, description = "Crate not found or download-restricted without access")
    ),
    security(("cargo_token" = []))
)]
pub async fn list_crate_versions(
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
    user: Result<maybe_user::MaybeUser, StatusCode>,
) -> ApiResult<Json<crate_version::CrateVersionList>> {
    let crate_name = crate_name.to_canonical();
    check_crate_visible(&crate_name, user.ok().as_ref(), &db).await?;

    let versions: Vec<crate_version::CrateVersion> = db
        .get_crate_versions(&crate_name)
        .await?
        .into_iter()
        .map(|v| crate_version::CrateVersion {
            version: v.to_string(),
        })
        .collect();

    Ok(Json(crate_version::CrateVersionList::from(versions)))
}

/// Search crates in Kellnr registry
///
/// Searches for crates by name and description in the Kellnr registry.
#[utoipa::path(
    get,
    path = "/",
    tag = "crates",
    params(
        ("q" = String, Query, description = "Search query"),
        ("page" = Option<u32>, Query, description = "Page to return, starting at 1"),
        ("per_page" = Option<u32>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Search results", body = SearchResult)
    ),
    security(("cargo_token" = []))
)]
pub async fn search(
    State(db): DbState,
    params: SearchParams,
    user: Result<maybe_user::MaybeUser, StatusCode>,
) -> ApiResult<Json<SearchResult>> {
    let limit = params.per_page.0;
    let offset = (params.page.0 - 1) * limit;
    let visibility = crate_visibility(user.ok().as_ref());
    let crates = db
        .search_in_crate_name_and_description(
            params.q.as_str(),
            limit as u64,
            offset as u64,
            false,
            &visibility,
        )
        .await?
        .into_iter()
        .map(|c| Crate {
            name: c.name,
            max_version: c.version,
            description: c
                .description
                .unwrap_or_else(|| "No description set".to_string()),
        })
        .collect::<Vec<Crate>>();

    let total = db
        .count_by_crate_name_and_description(params.q.as_str(), false, &visibility)
        .await?;

    Ok(Json(SearchResult {
        meta: search_result::Meta {
            total: total as i32,
        },
        crates,
    }))
}

/// Download a crate from Kellnr registry
///
/// Downloads a specific version of a crate from the Kellnr registry.
#[utoipa::path(
    get,
    path = "/dl/{package}/{version}/download",
    tag = "crates",
    params(
        ("package" = String, Path, description = "Package name"),
        ("version" = String, Path, description = "Package version")
    ),
    responses(
        (status = 200, description = "Crate archive", content_type = "application/octet-stream"),
        (status = 400, description = "Invalid package name or version"),
        (status = 403, description = "Download denied by a registry filter, e.g. the license filter"),
        (status = 404, description = "Crate not found or download-restricted without access")
    ),
    security(("cargo_token" = []))
)]
pub async fn download(
    State(state): AppState,
    token: token::OptionToken,
    Path((package, version)): Path<(OriginalName, Version)>,
) -> ApiResult<Response> {
    let db = state.db;
    let cs = state.crate_storage;
    let download_counter = state.download_counter;
    let canonical_name = package.to_canonical();
    check_download_auth(&canonical_name, &token, &db).await?;

    let file = cs
        .get(&package, &version)
        .await?
        .ok_or(RegistryError::CrateNotFound)?;

    let filters = &state.download_filters.registry;
    if !filters.is_empty() {
        let license = if let Some(known) = filters.licenses().get(&canonical_name, &version) {
            known.into_license()
        } else {
            let license = db.get_crate_license(&canonical_name, &version).await?;
            filters
                .licenses()
                .insert(&canonical_name, &version, license.clone().into());
            license
        };
        let krate = CrateInfo {
            name: &package,
            version: &version,
            license: license.as_deref(),
        };
        if let Some(message) = filters.check(&krate).deny_message(&krate) {
            warn!("{message}");
            return Ok((StatusCode::FORBIDDEN, message).into_response());
        }
    }

    // Only count downloads of crates that exist, so requests for made-up names
    // cannot fill the counter (immediate DB call when flush_interval=0).
    download_counter
        .increment_and_maybe_flush(canonical_name, version)
        .await;

    Ok(file.into_response())
}

/// Create an empty crate placeholder
///
/// Creates an empty crate placeholder for later publishing. Admin only.
#[utoipa::path(
    put,
    path = "/new_empty",
    tag = "crates",
    request_body = EmptyCrateData,
    responses(
        (status = 200, description = "Empty crate created", body = EmptyCrateSuccess),
        (status = 409, description = "Crate already exists"),
        (status = 401, description = "Unauthorized - admin only")
    ),
    security(("cargo_token" = []))
)]
pub async fn add_empty_crate(
    State(state): AppState,
    token: token::Token,
    Json(data): Json<EmptyCrateData>,
) -> ApiResult<Json<EmptyCrateSuccess>> {
    // Only admins can create empty crate placeholders
    if !token.is_admin {
        return Err(ApiError::new("Unauthorized", StatusCode::UNAUTHORIZED));
    }
    let db = state.db;
    let orig_name = OriginalName::try_from(&data.name)?;
    let canonical_name = orig_name.to_canonical();

    if let Some(existing) = db.get_crate_original_name(&canonical_name).await? {
        ensure_same_separators(&orig_name, &existing)?;
    }
    if let Some(id) = db.get_crate_id(&canonical_name).await? {
        let version = match db.get_max_version_from_id(id).await {
            Ok(v) => format!("{v}"),
            _ => String::new(),
        };
        return Err(RegistryError::CrateExists(data.name, version).into());
    }

    let created = Utc::now();

    // Add crate to DB
    db.add_empty_crate(&data.name, &created).await?;
    Ok(Json(EmptyCrateSuccess::new()))
}

/// Publish a new crate version
///
/// Publishes a new version of a crate to the Kellnr registry.
#[utoipa::path(
    put,
    path = "/new",
    tag = "crates",
    responses(
        (status = 200, description = "Crate published", body = PubDataSuccess),
        (status = 400, description = "Invalid crate data"),
        (status = 403, description = "Not an owner"),
        (status = 409, description = "Version exists"),
        (status = 413, description = "Crate is too large")
    ),
    security(("cargo_token" = []))
)]
pub async fn publish(
    State(state): AppState,
    token: token::Token,
    pub_data: PubData,
) -> ApiResult<Json<PubDataSuccess>> {
    let db = state.db;
    let settings = state.settings;
    let cs = state.crate_storage;
    let orig_name = OriginalName::try_from(&pub_data.metadata.name)?;
    let canonical_name = orig_name.to_canonical();

    let token_user = token.user.clone();
    // Shown in the version list. Publishing works without it, so a failed
    // lookup only leaves it empty.
    let token_name = match &token.value {
        Some(value) => db.get_auth_token_name(value).await.unwrap_or_else(|e| {
            warn!("Failed to look up the name of the publish token: {e}");
            None
        }),
        None => None,
    };
    let user = maybe_user::MaybeUser::from_token(token);

    // Check if user is read-only and can't publish (upload) crates.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&user)?;

    pub_data
        .metadata
        .validate_urls()
        .map_err(RegistryError::from)?;

    let version = Version::try_from(&pub_data.metadata.vers)?;

    // Check if user from token is an owner of the crate.
    // If not, they are not allowed to push a new version.
    // Check if crate with same version already exists.
    if let Some(existing) = db.get_crate_original_name(&canonical_name).await? {
        ensure_same_separators(&orig_name, &existing)?;
    }
    let id = db.get_crate_id(&canonical_name).await?;
    match (id, settings.registry.new_crates_restricted) {
        (Some(_), _) => {
            check_ownership(&canonical_name, &user, &db).await?;

            if let Some(existing) = find_same_version(&canonical_name, &version, &db).await? {
                return Err(RegistryError::CrateExists(pub_data.metadata.name, existing).into());
            }
        }
        (None, true) if !user.is_admin => {
            return Err(RegistryError::NewCratesRestricted.into());
        }

        _ => (),
    }

    // Check if required crate fields aren't present in crate
    // Skip serializing if no fields to check
    if !settings.registry.required_crate_fields.is_empty() {
        let serde_json::Value::Object(pkg_metadata) = serde_json::to_value(&pub_data.metadata)
            .map_err(RegistryError::InvalidMetadataString)?
        else {
            unreachable!()
        };

        let mut missing_fields = Vec::new();

        for field in &settings.registry.required_crate_fields {
            // If field is null or not present, complain
            if let Some(serde_json::Value::Null) | None = pkg_metadata.get(field) {
                missing_fields.push(field.clone());
            }
        }

        if !missing_fields.is_empty() {
            return Err(RegistryError::MissingRequiredFields(
                pub_data.metadata.name,
                missing_fields,
                settings.registry.required_crate_fields.clone(),
            )
            .into());
        }
    }

    let has_docs_link = pub_data.metadata.documentation.is_some();

    // Store the crate file and add it to the DB in a detached task. If the client
    // disconnects, the handler future gets dropped. Without the task, that could happen
    // between the storage write and the DB commit, leaving an orphaned crate file.
    let created = tokio::spawn({
        let db = db.clone();
        let cs = cs.clone();
        let orig_name = orig_name.clone();
        let version = version.clone();
        async move {
            let cratedata = pub_data.cratedata;
            let adopted = store_crate(&cs, &orig_name, &version, &cratedata).await?;
            let cksum = sha256::digest(&*cratedata);
            let created = Utc::now();

            if let Err(e) = db
                .add_crate(
                    &pub_data.metadata,
                    &cksum,
                    i64::from(pub_data.crate_length),
                    &created,
                    &token_user,
                    token_name,
                )
                .await
            {
                // An adopted file was not stored by this publish, so it is kept.
                if !adopted {
                    remove_stored_crate(&db, &cs, &orig_name, &version).await;
                }
                return Err(ApiError::from(e));
            }
            if adopted {
                restore_adopted_crate(&cs, &orig_name, &version, cratedata).await;
            }
            Ok(created)
        }
    })
    .await
    .map_err(|e| ApiError::internal(&e))??;

    // The crate is published at this point, so later failures are only logged
    // instead of telling cargo that the publish failed.
    kellnr_webhooks::notify_crate(
        if id.is_none() {
            WebhookEvent::CrateAdd
        } else {
            WebhookEvent::CrateUpdate
        },
        &created,
        &canonical_name,
        &version,
        &db,
    )
    .await;

    // Add crate to queue for doc extraction if there is no documentation value set already
    if settings.docs.enabled && !has_docs_link {
        match cs.create_rand_doc_queue_path().await {
            Ok(path) => {
                if let Err(e) = db.add_doc_queue(&orig_name, &version, &path).await {
                    error!("Failed to queue the docs build of {orig_name} {version}: {e}");
                }
            }
            Err(e) => {
                error!("Failed to create the docs queue path for {orig_name} {version}: {e}");
            }
        }
    }

    Ok(Json(PubDataSuccess::new()))
}

/// Store the crate file. Returns `true` if an orphaned file with the same
/// content already existed and is used instead. Such a file is left behind
/// if Kellnr stops between storing a crate and committing it to the database.
/// A file with a different content is never overwritten.
async fn store_crate(
    cs: &KellnrCrateStorage,
    name: &OriginalName,
    version: &Version,
    cratedata: &Bytes,
) -> ApiResult<bool> {
    match cs.put(name, version, cratedata.clone()).await {
        Ok(_) => Ok(false),
        Err(StorageError::CrateExists(_, _)) => match cs.get(name, version).await? {
            Some(existing) if existing == *cratedata => {
                warn!("Using the existing crate file of {name} {version} with the same content");
                Ok(true)
            }
            Some(_) => {
                error!(
                    "A different crate file of {name} {version} exists without a database entry"
                );
                Err(RegistryError::OrphanedCrateFile(name.to_string(), version.to_string()).into())
            }
            None => Err(ApiError::internal(&StorageError::GenericError(format!(
                "Failed to read the existing crate file of {name} {version}"
            )))),
        },
        Err(e) => Err(e.into()),
    }
}

/// Remove the crate file stored by a publish that failed to commit, unless a
/// concurrent publish of the same file committed the version in the meantime.
async fn remove_stored_crate(
    db: &Arc<dyn DbProvider>,
    cs: &KellnrCrateStorage,
    name: &OriginalName,
    version: &Version,
) {
    let committed = match db.get_crate_id(&name.to_canonical()).await {
        Ok(Some(id)) => db.crate_version_exists(id, version).await,
        Ok(None) => Ok(false),
        Err(e) => Err(e),
    };
    match committed {
        Ok(false) => {
            if let Err(e) = cs.delete(name, version).await {
                error!(
                    "Failed to remove the crate file of the failed publish of {name} {version}: {e}"
                );
            }
        }
        Ok(true) => {}
        Err(e) => {
            error!("Keeping the crate file of the failed publish of {name} {version}: {e}");
        }
    }
}

/// A concurrent publish of the same file can remove an adopted file if its
/// database commit failed, so it is stored again if it is missing.
async fn restore_adopted_crate(
    cs: &KellnrCrateStorage,
    name: &OriginalName,
    version: &Version,
    cratedata: Bytes,
) {
    match cs.put(name, version, cratedata).await {
        Ok(_) | Err(StorageError::CrateExists(_, _)) => {}
        Err(e) => error!("Failed to restore the crate file of {name} {version}: {e}"),
    }
}

/// Yank a crate version
///
/// Marks a crate version as yanked, preventing new downloads.
#[utoipa::path(
    delete,
    path = "/{crate_name}/{version}/yank",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name"),
        ("version" = String, Path, description = "Version to yank")
    ),
    responses(
        (status = 200, description = "Version yanked", body = YankSuccess),
        (status = 403, description = "Not an owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn yank(
    Path((crate_name, version)): Path<(OriginalName, Version)>,
    token: token::Token,
    State(db): DbState,
) -> ApiResult<Json<YankSuccess>> {
    // Check if user is read-only and can't yank crates.
    // Admin users bypass this check as they can modify
    // their read-only status.
    let user = maybe_user::MaybeUser::from_token(token);
    check_can_modify(&user)?;

    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, &db).await?;

    db.yank_crate(&crate_name, &version).await?;

    kellnr_webhooks::notify_crate(
        WebhookEvent::CrateYank,
        &Utc::now(),
        &crate_name,
        &version,
        &db,
    )
    .await;

    Ok(Json(YankSuccess::new()))
}

/// Unyank a crate version
///
/// Removes the yanked status from a crate version, allowing downloads again.
#[utoipa::path(
    put,
    path = "/{crate_name}/{version}/unyank",
    tag = "crates",
    params(
        ("crate_name" = String, Path, description = "Crate name"),
        ("version" = String, Path, description = "Version to unyank")
    ),
    responses(
        (status = 200, description = "Version unyanked", body = YankSuccess),
        (status = 403, description = "Not an owner")
    ),
    security(("cargo_token" = []))
)]
pub async fn unyank(
    Path((crate_name, version)): Path<(OriginalName, Version)>,
    token: token::Token,
    State(db): DbState,
) -> ApiResult<Json<YankSuccess>> {
    // Check if user is read-only and can't unyank crates.
    // Admin users bypass this check as they can modify
    // their read-only status.
    let user = maybe_user::MaybeUser::from_token(token);
    check_can_modify(&user)?;

    let crate_name = crate_name.to_canonical();
    check_ownership(&crate_name, &user, &db).await?;

    db.unyank_crate(&crate_name, &version).await?;

    kellnr_webhooks::notify_crate(
        WebhookEvent::CrateUnyank,
        &Utc::now(),
        &crate_name,
        &version,
        &db,
    )
    .await;

    Ok(Json(YankSuccess::new()))
}

#[cfg(test)]
mod reg_api_tests {
    use std::iter;
    use std::path::PathBuf;
    use std::time::Duration;

    use async_trait::async_trait;
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::{delete, get, put};
    use bytes::Bytes;
    use http_body_util::BodyExt;
    use hyper::header;
    use kellnr_appstate::AppStateData;
    use kellnr_common::publish_metadata::PublishMetadata;
    use kellnr_db::download_counter::DownloadCounter;
    use kellnr_db::mock::MockDb;
    use kellnr_db::{ConString, CrateVisibility, Database, SqliteConString, test_utils};
    use kellnr_error::api_error::ErrorDetails;
    use kellnr_settings::{LicenseUnknown, Settings};
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
    use kellnr_storage::storage::{ObjectWithMeta, Storage};
    use kellnr_storage::storage_error::StorageError;
    use kellnr_storage::storage_key::StorageKey;
    use mockall::predicate::*;
    use rand::distr::Alphanumeric;
    use rand::{RngExt, rng};
    use tokio::fs::read;
    use tokio::sync::Notify;
    use tower::ServiceExt;

    use super::*;

    const TOKEN: &str = "854DvwSlUwEHtIo3kWy6x7UCPKHfzCmy";
    const NON_ADMIN_TOKEN: &str = "g8kfzxSrMswNOVio5kBoTEFBBVm3fRS7";
    const RO_TOKEN: &str = "lJh6orU1Ye376ApXJR8I7V9gI3V6UZWU";
    const RO_ADMIN_TOKEN: &str = "GUOMPlZwN1kliXRW5wJ0ixh54NqYlE6X";

    /// Build a cargo publish body for `metadata` with a dummy crate file.
    fn pub_body(metadata: &PublishMetadata) -> Vec<u8> {
        let meta = serde_json::to_vec(metadata).unwrap();
        let crate_bytes = [0x1f, 0x8b, 0x08, 0x00];
        let mut buf = Vec::new();
        buf.extend_from_slice(&u32::try_from(meta.len()).unwrap().to_le_bytes());
        buf.extend_from_slice(&meta);
        buf.extend_from_slice(&u32::try_from(crate_bytes.len()).unwrap().to_le_bytes());
        buf.extend_from_slice(&crate_bytes);
        buf
    }

    async fn publish_metadata(
        kellnr: &TestKellnr,
        token: &str,
        metadata: &PublishMetadata,
    ) -> (StatusCode, String) {
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, token)
                    .body(Body::from(pub_body(metadata)))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = r.status();
        let body = r.into_body().collect().await.unwrap().to_bytes();
        let detail = serde_json::from_slice::<ErrorDetails>(&body)
            .map(|e| e.errors[0].detail.clone())
            .unwrap_or_default();
        (status, detail)
    }

    #[tokio::test]
    async fn publish_rejects_non_http_urls() {
        let kellnr = TestKellnr::new(get_settings()).await;

        for field in ["documentation", "homepage", "repository"] {
            let mut metadata = PublishMetadata::minimal("url_test", "1.0.0");
            let url = Some("javascript:alert(1)".to_string());
            match field {
                "documentation" => metadata.documentation = url,
                "homepage" => metadata.homepage = url,
                _ => metadata.repository = url,
            }

            let (status, detail) = publish_metadata(&kellnr, TOKEN, &metadata).await;

            assert_eq!(StatusCode::BAD_REQUEST, status);
            assert_eq!(
                format!(
                    "ERROR: URL for field `{field}` must begin with http:// or https:// (url: javascript:alert(1))"
                ),
                detail
            );
        }
        assert!(
            kellnr
                .db
                .get_crate_id(&CanonicalName::try_from("url_test").unwrap())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn publish_second_version_of_mixed_case_crate() {
        let kellnr = TestKellnr::new(get_settings()).await;

        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("MyCrate", "1.0.0"),
        )
        .await;
        assert_eq!(StatusCode::OK, status, "{detail}");

        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("MyCrate", "1.1.0"),
        )
        .await;
        assert_eq!(StatusCode::OK, status, "{detail}");

        let name = CanonicalName::try_from("mycrate").unwrap();
        assert_eq!(2, kellnr.db.get_crate_meta_list(&name).await.unwrap().len());
        assert_eq!(
            "1.1.0",
            kellnr
                .db
                .get_max_version_from_name(&name)
                .await
                .unwrap()
                .to_string()
        );
    }

    #[tokio::test]
    async fn publish_rejects_other_spelling_of_existing_crate() {
        let kellnr = TestKellnr::new(get_settings()).await;
        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("MyCrate", "1.0.0"),
        )
        .await;
        assert_eq!(StatusCode::OK, status, "{detail}");

        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("mycrate", "1.1.0"),
        )
        .await;

        assert_eq!(StatusCode::BAD_REQUEST, status);
        assert_eq!(
            "ERROR: Crate mycrate must be spelled like the existing crate MyCrate",
            detail
        );
        let name = CanonicalName::try_from("mycrate").unwrap();
        assert_eq!(1, kellnr.db.get_crate_meta_list(&name).await.unwrap().len());
    }

    /// Path of the crate file in the storage of the test settings.
    fn crate_file(settings: &Settings, name: &str, version: &str) -> PathBuf {
        settings.bin_path().join(format!("{name}-{version}.crate"))
    }

    /// The crate bytes `pub_body` sends.
    const CRATE_BYTES: [u8; 4] = [0x1f, 0x8b, 0x08, 0x00];

    #[tokio::test]
    async fn publish_adopts_orphaned_crate_file_with_same_content() {
        let settings = get_settings();
        let file = crate_file(&settings, "orphan", "1.0.0");
        let kellnr = TestKellnr::new(settings).await;
        std::fs::write(&file, CRATE_BYTES).unwrap();

        let (status, detail) =
            publish_metadata(&kellnr, TOKEN, &PublishMetadata::minimal("orphan", "1.0.0")).await;

        assert_eq!(StatusCode::OK, status, "{detail}");
        assert_eq!(CRATE_BYTES.to_vec(), std::fs::read(&file).unwrap());
        let name = CanonicalName::try_from("orphan").unwrap();
        assert_eq!(1, kellnr.db.get_crate_meta_list(&name).await.unwrap().len());
    }

    #[tokio::test]
    async fn publish_rejects_orphaned_crate_file_with_other_content() {
        let settings = get_settings();
        let file = crate_file(&settings, "orphan", "1.0.0");
        let kellnr = TestKellnr::new(settings).await;
        std::fs::write(&file, b"other").unwrap();

        let (status, detail) =
            publish_metadata(&kellnr, TOKEN, &PublishMetadata::minimal("orphan", "1.0.0")).await;

        assert_eq!(StatusCode::CONFLICT, status);
        assert_eq!(
            "ERROR: A different crate file of orphan 1.0.0 exists in the storage without being published. An administrator must remove the file orphan-1.0.0.crate from the crate storage",
            detail
        );
        assert_eq!(b"other".to_vec(), std::fs::read(&file).unwrap());
        let name = CanonicalName::try_from("orphan").unwrap();
        assert!(kellnr.db.get_crate_id(&name).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn failed_publish_keeps_adopted_crate_file() {
        let settings = get_settings();
        let file = crate_file(&settings, "foo_bar", "1.0.0");
        let kellnr = TestKellnr::new(settings).await;
        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("foo-bar", "1.0.0"),
        )
        .await;
        assert_eq!(StatusCode::OK, status, "{detail}");
        std::fs::write(&file, CRATE_BYTES).unwrap();

        // Fails in the database as the name only differs in '-' and '_'.
        let (status, _) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("foo_bar", "1.0.0"),
        )
        .await;

        assert_eq!(StatusCode::BAD_REQUEST, status);
        assert!(file.exists());
    }

    #[tokio::test]
    async fn failed_publish_removes_its_crate_file() {
        let settings = get_settings();
        let file = crate_file(&settings, "foo_bar", "1.0.0");
        let kellnr = TestKellnr::new(settings).await;
        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("foo-bar", "1.0.0"),
        )
        .await;
        assert_eq!(StatusCode::OK, status, "{detail}");

        let (status, _) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("foo_bar", "1.0.0"),
        )
        .await;

        assert_eq!(StatusCode::BAD_REQUEST, status);
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn publish_succeeds_when_queueing_the_docs_build_fails() {
        let mut settings = get_settings();
        settings.docs.enabled = true;
        let doc_queue = settings.doc_queue_path();
        let kellnr = TestKellnr::new(settings).await;
        // A file where the queue directory belongs makes queueing fail.
        let _ = std::fs::remove_dir_all(&doc_queue);
        std::fs::write(&doc_queue, b"").unwrap();

        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("docs_fail", "1.0.0"),
        )
        .await;

        assert_eq!(StatusCode::OK, status, "{detail}");
        let name = CanonicalName::try_from("docs_fail").unwrap();
        assert_eq!(1, kellnr.db.get_crate_meta_list(&name).await.unwrap().len());
    }

    #[tokio::test]
    async fn publish_records_the_publisher_and_token() {
        let kellnr = TestKellnr::new(get_settings()).await;
        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("recorded", "1.0.0"),
        )
        .await;
        assert_eq!(StatusCode::OK, status, "{detail}");

        let data = kellnr
            .db
            .get_crate_data(&CanonicalName::try_from("recorded").unwrap())
            .await
            .unwrap();
        assert_eq!(Some("admin".to_string()), data.versions[0].publisher);
        assert_eq!(Some("test".to_string()), data.versions[0].publisher_token);
    }

    #[tokio::test]
    async fn publish_rejects_name_differing_only_in_dash_and_underscore() {
        let kellnr = TestKellnr::new(get_settings()).await;
        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("foo-bar", "1.0.0"),
        )
        .await;
        assert_eq!(StatusCode::OK, status, "{detail}");

        let (status, detail) = publish_metadata(
            &kellnr,
            NON_ADMIN_TOKEN,
            &PublishMetadata::minimal("foo_bar", "1.0.0"),
        )
        .await;

        assert_eq!(StatusCode::BAD_REQUEST, status);
        assert_eq!(
            "ERROR: Crate name foo_bar is too similar to the existing crate foo-bar, names must not only differ in '-' and '_'",
            detail
        );
        let name = CanonicalName::try_from("foo_bar").unwrap();
        assert_eq!(
            Some(OriginalName::try_from("foo-bar").unwrap()),
            kellnr.db.get_crate_original_name(&name).await.unwrap()
        );
        assert_eq!(1, kellnr.db.get_crate_meta_list(&name).await.unwrap().len());
    }

    #[tokio::test]
    async fn crate_apis_find_crates_under_the_other_separator() {
        let kellnr = TestKellnr::new(get_settings()).await;
        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("foo-bar", "1.0.0"),
        )
        .await;
        assert_eq!(StatusCode::OK, status, "{detail}");
        let name = CanonicalName::try_from("foo-bar").unwrap();
        let is_yanked =
            || async { kellnr.db.get_crate_data(&name).await.unwrap().versions[0].yanked };

        for (request, yanked) in [
            (Request::get("/api/v1/crates/foo_bar/owners"), false),
            (Request::delete("/api/v1/crates/foo_bar/1.0.0/yank"), true),
            (Request::put("/api/v1/crates/Foo_Bar/1.0.0/unyank"), false),
        ] {
            let r = kellnr
                .client
                .clone()
                .oneshot(
                    request
                        .header(header::AUTHORIZATION, TOKEN)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(StatusCode::OK, r.status());
            assert_eq!(yanked, is_yanked().await);
        }
    }

    #[tokio::test]
    async fn add_empty_crate_rejects_similar_name() {
        let kellnr = TestKellnr::new(get_settings()).await;
        kellnr
            .db
            .add_empty_crate("foo-bar", &Utc::now())
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new_empty")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(r#"{"name":"foo_bar"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::BAD_REQUEST, r.status());
    }

    #[tokio::test]
    async fn publish_rejects_version_differing_only_in_build_metadata() {
        let kellnr = TestKellnr::new(get_settings()).await;
        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("build_meta", "1.0.0+a"),
        )
        .await;
        assert_eq!(StatusCode::OK, status, "{detail}");

        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("build_meta", "1.0.0+b"),
        )
        .await;

        assert_eq!(StatusCode::CONFLICT, status);
        assert_eq!(
            "ERROR: Crate with version already exists: build_meta-1.0.0+a",
            detail
        );
        let name = CanonicalName::try_from("build_meta").unwrap();
        assert_eq!(1, kellnr.db.get_crate_meta_list(&name).await.unwrap().len());
    }

    #[tokio::test]
    async fn publish_accepts_new_version_with_build_metadata() {
        let kellnr = TestKellnr::new(get_settings()).await;
        for vers in ["1.0.0+a", "1.0.1+a"] {
            let (status, detail) = publish_metadata(
                &kellnr,
                TOKEN,
                &PublishMetadata::minimal("build_meta", vers),
            )
            .await;
            assert_eq!(StatusCode::OK, status, "{detail}");
        }
    }

    #[tokio::test]
    async fn publish_accepts_http_and_empty_urls() {
        let kellnr = TestKellnr::new(get_settings()).await;
        let metadata = PublishMetadata {
            documentation: Some(String::new()),
            homepage: Some("http://example.com".to_string()),
            repository: Some("https://example.com/repo".to_string()),
            ..PublishMetadata::minimal("url_test", "1.0.0")
        };

        let (status, detail) = publish_metadata(&kellnr, TOKEN, &metadata).await;

        assert_eq!(StatusCode::OK, status, "{detail}");
    }

    async fn change_owners(kellnr: &TestKellnr, add: bool, users: &[&str]) -> (StatusCode, String) {
        let request = crate_user::CrateUserRequest {
            users: users.iter().map(ToString::to_string).collect(),
        };
        let builder = if add {
            Request::put("/api/v1/crates/owned/owners")
        } else {
            Request::delete("/api/v1/crates/owned/owners")
        };
        let r = kellnr
            .client
            .clone()
            .oneshot(
                builder
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(serde_json::to_string(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = r.status();
        let body = r.into_body().collect().await.unwrap().to_bytes();
        let detail = serde_json::from_slice::<ErrorDetails>(&body)
            .map(|e| e.errors[0].detail.clone())
            .unwrap_or_default();
        (status, detail)
    }

    async fn owner_names(kellnr: &TestKellnr) -> Vec<String> {
        let mut names: Vec<String> = kellnr
            .db
            .get_crate_owners(&CanonicalName::try_from("owned").unwrap())
            .await
            .unwrap()
            .into_iter()
            .map(|u| u.name)
            .collect();
        names.sort();
        names
    }

    async fn crate_with_two_owners() -> TestKellnr {
        let kellnr = TestKellnr::new(get_settings()).await;
        let (status, detail) =
            publish_metadata(&kellnr, TOKEN, &PublishMetadata::minimal("owned", "1.0.0")).await;
        assert_eq!(StatusCode::OK, status, "{detail}");
        let (status, detail) = change_owners(&kellnr, true, &["non_admin"]).await;
        assert_eq!(StatusCode::OK, status, "{detail}");
        kellnr
    }

    #[tokio::test]
    async fn add_existing_owner_succeeds() {
        let kellnr = crate_with_two_owners().await;

        let (status, detail) = change_owners(&kellnr, true, &["non_admin"]).await;

        assert_eq!(StatusCode::OK, status, "{detail}");
        assert_eq!(vec!["admin", "non_admin"], owner_names(&kellnr).await);
    }

    #[tokio::test]
    async fn remove_owners_with_a_non_owner_removes_nothing() {
        let kellnr = crate_with_two_owners().await;

        let (status, detail) = change_owners(&kellnr, false, &["non_admin", "ro_dummy"]).await;

        assert_eq!(StatusCode::NOT_FOUND, status);
        assert_eq!("ERROR: Owner not found: ro_dummy", detail);
        assert_eq!(vec!["admin", "non_admin"], owner_names(&kellnr).await);
    }

    #[tokio::test]
    async fn remove_owners_listed_twice_keeps_the_other_owner() {
        let kellnr = crate_with_two_owners().await;

        let (status, detail) = change_owners(&kellnr, false, &["non_admin", "non_admin"]).await;

        assert_eq!(StatusCode::OK, status, "{detail}");
        assert_eq!(vec!["admin"], owner_names(&kellnr).await);
    }

    #[tokio::test]
    async fn remove_all_owners_is_rejected() {
        let kellnr = crate_with_two_owners().await;

        let (status, detail) = change_owners(&kellnr, false, &["admin", "non_admin"]).await;

        assert_eq!(StatusCode::CONFLICT, status);
        assert_eq!("ERROR: A crate must have at least one owner", detail);
        assert_eq!(vec!["admin", "non_admin"], owner_names(&kellnr).await);
    }

    // Test that removal of the last owner is prevented with default settings.
    #[tokio::test]
    async fn remove_owner_valid_owner_is_rejected() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let del_owner = crate_user::CrateUserRequest {
            users: vec!["admin".to_string()],
        };
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test_lib/owners")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(serde_json::to_string(&del_owner).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();

        assert_eq!(
            1,
            kellnr
                .db
                .get_crate_owners(&CanonicalName::try_from("test_lib").unwrap())
                .await
                .unwrap()
                .len()
        );

        assert_eq!(status, StatusCode::CONFLICT);
        let owners = serde_json::from_slice::<ErrorDetails>(&result_msg).unwrap();
        assert_eq!(
            "ERROR: A crate must have at least one owner",
            owners.errors[0].detail
        );
    }

    // Test that removal of the last owner is possible
    // when `allow_ownerless_crate` setting is enabled.
    #[tokio::test]
    async fn remove_owner_valid_owner_is_accepted() {
        let mut settings = get_settings();
        settings.registry.allow_ownerless_crates = true;
        let kellnr = TestKellnr::new(settings).await;

        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let del_owner = crate_user::CrateUserRequest {
            users: vec!["admin".to_string()],
        };
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test_lib/owners")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(serde_json::to_string(&del_owner).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = r.status();

        assert!(
            kellnr
                .db
                .get_crate_owners(&CanonicalName::try_from("test_lib").unwrap())
                .await
                .unwrap()
                .is_empty()
        );

        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn add_owner_valid_owner() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();
        kellnr
            .db
            .add_user("user", "123", false, false)
            .await
            .unwrap();
        let add_owner = crate_user::CrateUserRequest {
            users: vec!["user".to_string()],
        };

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test_lib/owners")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(serde_json::to_string(&add_owner).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let owners = serde_json::from_slice::<crate_user::CrateUserResponse>(&result_msg).unwrap();
        assert!(owners.ok);
    }

    #[tokio::test]
    async fn add_owner_single_valid_owner() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        kellnr
            .db
            .add_user("user", "123", false, false)
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test_lib/owners/user")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);

        let owners = kellnr
            .db
            .get_crate_owners(&CanonicalName::try_from("test_lib").unwrap())
            .await
            .unwrap();
        assert_eq!(2, owners.len());
    }

    // Test that removal of the last owner is prevented with default settings.
    #[tokio::test]
    async fn remove_owner_single_last_owner_is_rejected() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test_lib/owners/admin")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::CONFLICT);
    }

    // Test that removal of the last owner is possible
    // when `allow_ownerless_crate` setting is enabled.
    #[tokio::test]
    async fn remove_owner_single_last_owner_is_accepted() {
        let mut settings = get_settings();
        settings.registry.allow_ownerless_crates = true;
        let kellnr = TestKellnr::new(settings).await;

        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test_lib/owners/admin")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);

        let owners = kellnr
            .db
            .get_crate_owners(&CanonicalName::try_from("test_lib").unwrap())
            .await
            .unwrap();
        assert!(owners.is_empty());
    }

    #[tokio::test]
    async fn add_owner_single_non_owner_is_forbidden() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // publish crate as admin
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // create regular user and token
        kellnr
            .db
            .add_user("user", "123", false, false)
            .await
            .unwrap();
        kellnr
            .db
            .add_auth_token("user token", "user_token", "user")
            .await
            .unwrap();

        // try to add an owner as non-owner
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test_lib/owners/admin")
                    .header(header::AUTHORIZATION, "user_token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn remove_owner_single_non_owner_is_forbidden() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // publish crate as admin
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // create regular user and token
        kellnr
            .db
            .add_user("user", "123", false, false)
            .await
            .unwrap();
        kellnr
            .db
            .add_auth_token("user token", "user_token", "user")
            .await
            .unwrap();

        // try to remove the owner as non-owner
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test_lib/owners/admin")
                    .header(header::AUTHORIZATION, "user_token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn add_owner_bulk_non_owner_is_forbidden() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // publish crate as admin
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // create regular user and token
        kellnr
            .db
            .add_user("user", "123", false, false)
            .await
            .unwrap();
        kellnr
            .db
            .add_auth_token("user token", "user_token", "user")
            .await
            .unwrap();

        let add_owner = crate_user::CrateUserRequest {
            users: vec!["admin".to_string()],
        };

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test_lib/owners")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, "user_token")
                    .body(Body::from(serde_json::to_string(&add_owner).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn remove_owner_bulk_non_owner_is_forbidden() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // publish crate as admin
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // create regular user and token
        kellnr
            .db
            .add_user("user", "123", false, false)
            .await
            .unwrap();
        kellnr
            .db
            .add_auth_token("user token", "user_token", "user")
            .await
            .unwrap();

        let del_owner = crate_user::CrateUserRequest {
            users: vec!["admin".to_string()],
        };

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test_lib/owners")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, "user_token")
                    .body(Body::from(serde_json::to_string(&del_owner).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn add_owner_single_non_owner_session_is_forbidden() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // publish crate as admin
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // create regular user
        kellnr
            .db
            .add_user("user", "123", false, false)
            .await
            .unwrap();

        // create a session token and store it in DB
        kellnr.db.add_session_token("user", "1234").await.unwrap();

        // create signed cookie header
        let cookie_header = kellnr_appstate::encode_cookies([(
            kellnr_settings::constants::COOKIE_SESSION_ID,
            "1234",
        )]);

        // try to add owner via session as non-owner
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test_lib/owners/admin")
                    .header(header::COOKIE, cookie_header)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Session now decrypts and validates: a logged-in non-owner is
        // rejected by the ownership check (403), not treated as anonymous.
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }

    // A read-only user who is an owner must not be able to modify a crate
    // through the web session. Read-only state is now resolved for session
    // auth, so `check_can_modify` rejects the request.
    #[tokio::test]
    async fn add_owner_single_read_only_owner_session_is_forbidden() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // publish crate as admin so `test_lib` exists
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // create a read-only user and make them an owner of the crate
        kellnr
            .db
            .add_user("rouser", "123", false, true)
            .await
            .unwrap();
        kellnr
            .db
            .add_owner(&CanonicalName::try_from("test_lib").unwrap(), "rouser")
            .await
            .unwrap();

        // create a session for the read-only owner
        kellnr.db.add_session_token("rouser", "1234").await.unwrap();
        let cookie_header = kellnr_appstate::encode_cookies([(
            kellnr_settings::constants::COOKIE_SESSION_ID,
            "1234",
        )]);

        // read-only owner tries to add an owner via the browser session
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test_lib/owners/admin")
                    .header(header::COOKIE, cookie_header)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // ReadOnlyModify maps to 400 Bad Request
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    }

    // Counterpart to the read-only test: a normal (writable) owner must still
    // be able to modify the crate through the web session.
    #[tokio::test]
    async fn add_owner_single_writable_owner_session_is_allowed() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // publish crate as admin so `test_lib` exists
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // a writable owner and a target user to be added as owner
        kellnr
            .db
            .add_user("rwowner", "123", false, false)
            .await
            .unwrap();
        kellnr
            .db
            .add_owner(&CanonicalName::try_from("test_lib").unwrap(), "rwowner")
            .await
            .unwrap();
        kellnr
            .db
            .add_user("target", "123", false, false)
            .await
            .unwrap();

        // create a session for the writable owner
        kellnr
            .db
            .add_session_token("rwowner", "1234")
            .await
            .unwrap();
        let cookie_header = kellnr_appstate::encode_cookies([(
            kellnr_settings::constants::COOKIE_SESSION_ID,
            "1234",
        )]);

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test_lib/owners/target")
                    .header(header::COOKIE, cookie_header)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn remove_owner_single_non_owner_session_is_forbidden() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // publish crate as admin
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // create regular user
        kellnr
            .db
            .add_user("user", "123", false, false)
            .await
            .unwrap();

        // create a session token and store it in DB
        kellnr.db.add_session_token("user", "1234").await.unwrap();

        let cookie_header = kellnr_appstate::encode_cookies([(
            kellnr_settings::constants::COOKIE_SESSION_ID,
            "1234",
        )]);

        // try to remove owner via session as non-owner
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test_lib/owners/admin")
                    .header(header::COOKIE, cookie_header)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Session now decrypts and validates: a logged-in non-owner is
        // rejected by the ownership check (403), not treated as anonymous.
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn list_owners_valid_owner() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/crates/test_lib/owners")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();

        let owners = serde_json::from_slice::<crate_user::CrateUserList>(&result_msg).unwrap();
        assert_eq!(1, owners.users.len());
        assert_eq!("admin", owners.users[0].login);
    }

    #[tokio::test]
    async fn publish_garbage() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;

        let garbage = vec![0x00, 0x11, 0x22, 0x33];
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(garbage))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response_status = r.status();
        let error: ErrorDetails =
            serde_json::from_slice(r.into_body().collect().await.unwrap().to_bytes().as_ref())
                .expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            "ERROR: Invalid min. length 4/10 bytes",
            error.errors[0].detail
        );
    }

    #[tokio::test]
    async fn download_not_existing_package() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/cratesio/does_not_exist/0.1.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn download_counts_only_existing_versions() {
        let kellnr = TestKellnr::new(get_settings()).await;
        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("counted", "1.0.0"),
        )
        .await;
        assert_eq!(StatusCode::OK, status, "{detail}");

        for (version, expected) in [("9.9.9", StatusCode::NOT_FOUND), ("1.0.0", StatusCode::OK)] {
            let r = kellnr
                .client
                .clone()
                .oneshot(
                    Request::get(format!("/api/v1/crates/counted/{version}/download"))
                        .header(header::AUTHORIZATION, TOKEN)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(expected, r.status());
        }

        assert_eq!(1, kellnr.db.get_total_downloads().await.unwrap());
    }

    async fn download_with_registry_filter(
        unknown: LicenseUnknown,
        license: Option<&str>,
    ) -> (StatusCode, String) {
        let settings = Settings {
            registry_filter: kellnr_settings::RegistryFilter {
                license_enabled: true,
                license_allowed: vec!["MIT".to_string()],
                license_unknown: unknown,
                license_exempt_crates: vec![],
            },
            ..get_settings()
        };
        let kellnr = TestKellnr::new(settings).await;
        let mut metadata = PublishMetadata::minimal("filtered", "1.0.0");
        metadata.license = license.map(ToString::to_string);
        let (status, detail) = publish_metadata(&kellnr, TOKEN, &metadata).await;
        // Publishing is never blocked by the filter
        assert_eq!(StatusCode::OK, status, "{detail}");

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/crates/filtered/1.0.0/download")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = r.status();
        let body = r.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&body).to_string())
    }

    #[tokio::test]
    async fn registry_filter_allows_allowed_license() {
        let (status, _) =
            download_with_registry_filter(LicenseUnknown::Deny, Some("MIT OR Apache-2.0")).await;

        assert_eq!(StatusCode::OK, status);
    }

    #[tokio::test]
    async fn registry_filter_denies_license() {
        let (status, body) =
            download_with_registry_filter(LicenseUnknown::Deny, Some("GPL-3.0-only")).await;

        assert_eq!(StatusCode::FORBIDDEN, status);
        assert_eq!(
            "Download of filtered@1.0.0 denied by Kellnr license filter: license \"GPL-3.0-only\" is not allowed",
            body
        );
    }

    #[tokio::test]
    async fn registry_filter_treats_missing_license_as_unknown() {
        let (deny, _) = download_with_registry_filter(LicenseUnknown::Deny, None).await;
        let (allow, _) = download_with_registry_filter(LicenseUnknown::Allow, None).await;

        assert_eq!(StatusCode::FORBIDDEN, deny);
        assert_eq!(StatusCode::OK, allow);
    }

    #[tokio::test]
    async fn download_invalid_package_name() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/cratesio/-invalid_name/0.1.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn download_not_existing_version() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/crates/test-lib/99.1.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn download_invalid_package_version() {
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/crates/invalid_version/0.a.0/download")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Invalid semver is rejected during path deserialization (400 Bad Request)
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn search_verify_query_and_default() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_search_in_crate_name_and_description()
            .with(
                eq("foo"),
                eq(10),
                eq(0),
                eq(false),
                eq(CrateVisibility::Unrestricted),
            )
            .returning(|_, _, _, _, _| Ok(vec![]));
        mock_db
            .expect_count_by_crate_name_and_description()
            .with(eq("foo"), eq(false), eq(CrateVisibility::Unrestricted))
            .returning(|_, _, _| Ok(0));

        let kellnr = app_search(Arc::new(mock_db));
        let r = kellnr
            .oneshot(
                Request::get("/api/v1/crates?q=foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<SearchResult>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn search_with_token_uses_user_visibility() {
        let mut mock_db = restricted_crate_mock_db();
        mock_db
            .expect_search_in_crate_name_and_description()
            .with(
                eq("foo"),
                eq(10),
                eq(0),
                eq(false),
                eq(CrateVisibility::User("crate_user".to_string())),
            )
            .returning(|_, _, _, _, _| Ok(vec![]));
        mock_db
            .expect_count_by_crate_name_and_description()
            .with(
                eq("foo"),
                eq(false),
                eq(CrateVisibility::User("crate_user".to_string())),
            )
            .returning(|_, _, _| Ok(0));

        let r = app_search(Arc::new(mock_db))
            .oneshot(
                Request::get("/api/v1/crates?q=foo")
                    .header(header::AUTHORIZATION, "crate-user-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, r.status());
    }

    /// Mock with the download-restricted crate `secret` that only the token
    /// `crate-user-token` of user `crate_user` may access. The token
    /// `other-token` of user `other` has no access. No other crate exists.
    fn restricted_crate_mock_db() -> MockDb {
        let secret = CanonicalName::try_from("secret").unwrap();
        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_crate_id()
            .returning(|name| Ok((name.as_str() == "secret").then_some(1)));
        mock_db
            .expect_is_download_restricted()
            .with(eq(secret.clone()))
            .returning(|_| Ok(true));
        mock_db
            .expect_is_download_restricted()
            .returning(|_| Ok(false));
        for (token, user) in [("crate-user-token", "crate_user"), ("other-token", "other")] {
            mock_db
                .expect_get_user_from_token()
                .with(eq(token))
                .returning(move |_| {
                    Ok(kellnr_db::User {
                        name: user.to_string(),
                        ..kellnr_db::User::default()
                    })
                });
        }
        mock_db
            .expect_is_crate_user()
            .with(eq(secret), eq("crate_user"))
            .returning(|_, _| Ok(true));
        mock_db.expect_is_crate_user().returning(|_, _| Ok(false));
        mock_db
            .expect_is_crate_group_user()
            .returning(|_, _| Ok(false));
        mock_db.expect_is_owner().returning(|_, _| Ok(false));
        mock_db.expect_get_crate_owners().returning(|_| Ok(vec![]));
        mock_db
            .expect_get_crate_versions()
            .returning(|_| Ok(vec![]));
        mock_db
    }

    async fn get_restricted(uri: &str, token: Option<&str>) -> StatusCode {
        let mut request = Request::get(uri);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, token);
        }
        Router::new()
            .route("/api/v1/crates/{crate_name}/owners", get(list_owners))
            .route(
                "/api/v1/crates/{crate_name}/crate_versions",
                get(list_crate_versions),
            )
            .route(
                "/api/v1/crates/dl/{package}/{version}/download",
                get(download),
            )
            .with_state(AppStateData {
                db: Arc::new(restricted_crate_mock_db()),
                ..kellnr_appstate::test_state()
            })
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn list_owners_restricted_crate_anonymous_is_not_found() {
        assert_eq!(
            StatusCode::NOT_FOUND,
            get_restricted("/api/v1/crates/secret/owners", None).await
        );
    }

    #[tokio::test]
    async fn list_owners_restricted_crate_crate_user_is_ok() {
        assert_eq!(
            StatusCode::OK,
            get_restricted("/api/v1/crates/secret/owners", Some("crate-user-token")).await
        );
    }

    #[tokio::test]
    async fn list_crate_versions_restricted_crate_anonymous_is_not_found() {
        assert_eq!(
            StatusCode::NOT_FOUND,
            get_restricted("/api/v1/crates/secret/crate_versions", None).await
        );
    }

    #[tokio::test]
    async fn list_owners_missing_crate_is_not_found() {
        assert_eq!(
            StatusCode::NOT_FOUND,
            get_restricted("/api/v1/crates/missing/owners", Some("other-token")).await
        );
    }

    #[tokio::test]
    async fn list_crate_versions_missing_crate_is_not_found() {
        assert_eq!(
            StatusCode::NOT_FOUND,
            get_restricted("/api/v1/crates/missing/crate_versions", Some("other-token")).await
        );
    }

    #[tokio::test]
    async fn download_restricted_crate_anonymous_is_not_found() {
        assert_eq!(
            StatusCode::NOT_FOUND,
            get_restricted("/api/v1/crates/dl/secret/1.0.0/download", None).await
        );
    }

    #[tokio::test]
    async fn download_restricted_crate_without_access_is_not_found() {
        assert_eq!(
            StatusCode::NOT_FOUND,
            get_restricted(
                "/api/v1/crates/dl/secret/1.0.0/download",
                Some("other-token")
            )
            .await
        );
    }

    #[tokio::test]
    async fn list_crate_versions_restricted_crate_crate_user_is_ok() {
        assert_eq!(
            StatusCode::OK,
            get_restricted(
                "/api/v1/crates/secret/crate_versions",
                Some("crate-user-token")
            )
            .await
        );
    }

    #[tokio::test]
    async fn search_verify_page() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_search_in_crate_name_and_description()
            .with(
                eq("foo"),
                eq(20),
                eq(40),
                eq(false),
                eq(CrateVisibility::Unrestricted),
            )
            .returning(|_, _, _, _, _| Ok(vec![]));
        mock_db
            .expect_count_by_crate_name_and_description()
            .with(eq("foo"), eq(false), eq(CrateVisibility::Unrestricted))
            .returning(|_, _, _| Ok(0));

        let kellnr = app_search(Arc::new(mock_db));
        let r = kellnr
            .oneshot(
                Request::get("/api/v1/crates?q=foo&page=3&per_page=20")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<SearchResult>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn search_verify_per_page() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_search_in_crate_name_and_description()
            .with(
                eq("foo"),
                eq(20),
                eq(0),
                eq(false),
                eq(CrateVisibility::Unrestricted),
            )
            .returning(|_, _, _, _, _| Ok(vec![]));
        mock_db
            .expect_count_by_crate_name_and_description()
            .with(eq("foo"), eq(false), eq(CrateVisibility::Unrestricted))
            .returning(|_, _, _| Ok(0));

        let kellnr = app_search(Arc::new(mock_db));
        let r = kellnr
            .oneshot(
                Request::get("/api/v1/crates?q=foo&per_page=20")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<SearchResult>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn search_verify_total_is_count() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_search_in_crate_name_and_description()
            .with(
                eq("foo"),
                eq(20),
                eq(0),
                eq(false),
                eq(CrateVisibility::Unrestricted),
            )
            .returning(|_, _, _, _, _| Ok(vec![]));
        mock_db
            .expect_count_by_crate_name_and_description()
            .with(eq("foo"), eq(false), eq(CrateVisibility::Unrestricted))
            .returning(|_, _, _| Ok(1234));

        let kellnr = app_search(Arc::new(mock_db));
        let r = kellnr
            .oneshot(
                Request::get("/api/v1/crates?q=foo&per_page=20")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let res = serde_json::from_slice::<SearchResult>(&result_msg).unwrap();
        assert_eq!(1234, res.meta.total);
    }

    #[tokio::test]
    async fn search_verify_page_out_of_range() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/crates?q=foo&page=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::BAD_REQUEST, r.status());
    }

    #[tokio::test]
    async fn search_verify_per_page_out_of_range() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/crates?q=foo&per_page=200")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::BAD_REQUEST, r.status());
    }

    #[tokio::test]
    async fn yank_success() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test_lib/0.2.0/yank")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<YankSuccess>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn yank_error() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::delete("/api/v1/crates/test/0.1.0/yank")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<ErrorDetails>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn unyank_success() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test_lib/0.2.0/unyank")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<YankSuccess>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn unyank_error() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/test/0.1.0/unyank")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<ErrorDetails>(&result_msg).is_ok());
    }

    #[tokio::test]
    async fn add_empty_package() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new_empty")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from("{\"name\": \"next-crate\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let _success: EmptyCrateSuccess =
            serde_json::from_slice(&result_msg).expect("Cannot deserialize success message");

        assert_eq!(StatusCode::OK, response_status);
        let canonical_name = OriginalName::try_from("next-crate").unwrap().to_canonical();
        let crate_id = kellnr
            .db
            .get_crate_id(&canonical_name)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            Version::try_from("0.0.0").unwrap(),
            kellnr.db.get_max_version_from_id(crate_id).await.unwrap()
        );
    }

    #[tokio::test]
    async fn add_empty_non_admin() {
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new_empty")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, NON_ADMIN_TOKEN)
                    .body(Body::from("{\"name\": \"rogue-crate\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response_status = r.status();

        assert_eq!(StatusCode::UNAUTHORIZED, response_status);

        let canonical_name = OriginalName::try_from("rogue-crate")
            .unwrap()
            .to_canonical();
        assert_eq!(None, kellnr.db.get_crate_id(&canonical_name).await.unwrap());
    }

    #[tokio::test]
    async fn add_empty_existing() {
        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new_empty")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from("{\"name\": \"test_lib\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();

        let error: ErrorDetails =
            serde_json::from_slice(&result_msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::CONFLICT, response_status);
        assert_eq!(
            "ERROR: Crate with version already exists: test_lib-0.2.0",
            error.errors[0].detail
        );
    }

    #[tokio::test]
    async fn publish_package() {
        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let success: PubDataSuccess =
            serde_json::from_slice(&result_msg).expect("Cannot deserialize success message");

        assert_eq!(StatusCode::OK, response_status);
        assert!(success.warnings.is_none());
        // As the success message is empty in the normal case, the deserialization works even
        // if an error message was returned. That's why we need to test for an error message, too.
        assert!(
            serde_json::from_slice::<ErrorDetails>(&result_msg).is_err(),
            "An error message instead of a success message was returned"
        );
        assert_eq!(
            1,
            test_utils::get_crate_meta_list(&kellnr.db, 1)
                .await
                .unwrap()
                .len()
        );
        assert_eq!(
            "0.2.0",
            test_utils::get_crate_meta_list(&kellnr.db, 1)
                .await
                .unwrap()[0]
                .version
        );
    }

    #[tokio::test]
    async fn try_publish_as_read_only_non_admin() {
        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, RO_TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response_status = r.status();
        let msg = r.into_body().collect().await.unwrap().to_bytes();

        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            "ERROR: Read-only users cannot modify the registry",
            error.errors[0].detail
        );
    }

    #[tokio::test]
    async fn try_publish_as_read_only_admin() {
        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let settings = get_settings();
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, RO_ADMIN_TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let success: PubDataSuccess =
            serde_json::from_slice(&result_msg).expect("Cannot deserialize success message");

        assert_eq!(StatusCode::OK, response_status);
        assert!(success.warnings.is_none());
        // As the success message is empty in the normal case, the deserialization works even
        // if an error message was returned. That's why we need to test for an error message, too.
        assert!(
            serde_json::from_slice::<ErrorDetails>(&result_msg).is_err(),
            "An error message instead of a success message was returned"
        );
        assert_eq!(
            1,
            test_utils::get_crate_meta_list(&kellnr.db, 1)
                .await
                .unwrap()
                .len()
        );
        assert_eq!(
            "0.2.0",
            test_utils::get_crate_meta_list(&kellnr.db, 1)
                .await
                .unwrap()[0]
                .version
        );
    }

    #[tokio::test]
    async fn try_publish_with_restricted() {
        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut settings = get_settings();
        settings.registry.new_crates_restricted = true;
        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, NON_ADMIN_TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response_status = r.status();
        let msg = r.into_body().collect().await.unwrap().to_bytes();

        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            "ERROR: New crates publishing has been restricted",
            error.errors[0].detail
        );
    }

    #[tokio::test]
    async fn try_publish_with_restricted_added_empty() {
        let mut settings = get_settings();
        settings.registry.new_crates_restricted = true;
        let kellnr = TestKellnr::fake(settings).await;

        // add empty crate placeholder
        let created = Utc::now();
        kellnr
            .db
            .add_empty_crate("test_lib", &created)
            .await
            .unwrap();
        // add the non_admin user as the owner
        let canonical_name = OriginalName::try_from("test_lib").unwrap().to_canonical();
        kellnr
            .db
            .add_owner(&canonical_name, "non_admin")
            .await
            .unwrap();

        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, NON_ADMIN_TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        let success: PubDataSuccess =
            serde_json::from_slice(&result_msg).expect("Cannot deserialize success message");

        assert_eq!(StatusCode::OK, response_status);
        assert!(success.warnings.is_none());
        // As the success message is empty in the normal case, the deserialization works even
        // if an error message was returned. That's why we need to test for an error message, too.
        assert!(
            serde_json::from_slice::<ErrorDetails>(&result_msg).is_err(),
            "An error message instead of a success message was returned"
        );
        assert_eq!(
            1,
            kellnr
                .db
                .get_crate_meta_list(&canonical_name)
                .await
                .unwrap()
                .len()
        );
        assert_eq!(
            "0.2.0",
            kellnr
                .db
                .get_crate_meta_list(&canonical_name)
                .await
                .unwrap()[0]
                .version
        );
    }

    #[tokio::test]
    async fn try_publish_with_restricted_added_empty_non_owner() {
        let mut settings = get_settings();
        settings.registry.new_crates_restricted = true;
        let kellnr = TestKellnr::fake(settings).await;

        let created = Utc::now();
        kellnr
            .db
            .add_empty_crate("test_lib", &created)
            .await
            .unwrap();

        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, NON_ADMIN_TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response_status = r.status();
        let msg = r.into_body().collect().await.unwrap().to_bytes();

        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::FORBIDDEN, response_status);
        assert_eq!("ERROR: Not the owner of the crate", error.errors[0].detail);
    }

    #[tokio::test]
    async fn publish_crate_with_missing_one_required_field() {
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut settings = get_settings();
        settings.registry.required_crate_fields = vec!["repository".to_string()];

        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let msg = r.into_body().collect().await.unwrap().to_bytes();

        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            r#"ERROR: Required field(s) not defined for crate test_lib, missing: ["repository"], requires: ["repository"]"#,
            error.errors[0].detail
        );
    }

    #[tokio::test]
    async fn publish_crate_with_missing_multiple_required_fields() {
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut settings = get_settings();
        settings.registry.required_crate_fields =
            vec!["repository".to_string(), "license".to_string()];

        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let msg = r.into_body().collect().await.unwrap().to_bytes();

        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            r#"ERROR: Required field(s) not defined for crate test_lib, missing: ["repository", "license"], requires: ["repository", "license"]"#,
            error.errors[0].detail
        );
    }

    // Missing some but not all required fields
    #[tokio::test]
    async fn publish_crate_with_some_required_fields() {
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let mut settings = get_settings();
        settings.registry.required_crate_fields =
            vec!["repository".to_string(), "authors".to_string()];

        let kellnr = TestKellnr::fake(settings).await;
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Get the empty success results message.
        let response_status = r.status();
        let msg = r.into_body().collect().await.unwrap().to_bytes();

        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            r#"ERROR: Required field(s) not defined for crate test_lib, missing: ["repository"], requires: ["repository", "authors"]"#,
            error.errors[0].detail
        );
    }

    #[tokio::test]
    async fn publish_existing_package() {
        // Use valid crate publish data to test.
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let settings = get_settings();
        let kellnr = TestKellnr::new(settings).await;
        let _ = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Publish same package a second time.
        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            )
            .await
            .unwrap();
        let response_status = r.status();

        let msg = r.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorDetails =
            serde_json::from_slice(&msg).expect("Cannot deserialize error message");

        assert_eq!(StatusCode::CONFLICT, response_status);
        assert_eq!(
            "ERROR: Crate with version already exists: test_lib-0.2.0",
            error.errors[0].detail
        );
    }

    /// Storage that signals `stored` once a crate file is written and then holds
    /// `put` until `release` is notified, to simulate a slow publish.
    struct GatedStorage {
        inner: FSStorage,
        stored: Arc<Notify>,
        release: Arc<Notify>,
    }

    #[async_trait]
    impl Storage for GatedStorage {
        async fn get(&self, key: &StorageKey) -> Result<Bytes, StorageError> {
            self.inner.get(key).await
        }

        async fn get_with_meta(&self, key: &StorageKey) -> Result<ObjectWithMeta, StorageError> {
            self.inner.get_with_meta(key).await
        }

        async fn put(&self, key: &StorageKey, object: Bytes) -> Result<(), StorageError> {
            self.inner.put(key, object).await?;
            self.stored.notify_one();
            self.release.notified().await;
            Ok(())
        }

        async fn put_overwrite(&self, key: &StorageKey, object: Bytes) -> Result<(), StorageError> {
            self.inner.put_overwrite(key, object).await
        }

        async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
            self.inner.delete(key).await
        }

        async fn exists(&self, key: &StorageKey) -> Result<bool, StorageError> {
            self.inner.exists(key).await
        }

        async fn size(&self, key: &StorageKey) -> Result<u64, StorageError> {
            self.inner.size(key).await
        }

        async fn list(&self, prefix: &str) -> Result<Vec<StorageKey>, StorageError> {
            self.inner.list(prefix).await
        }
    }

    /// Storage that cannot read crate files, like a remote storage that times out.
    struct UnreadableStorage(FSStorage);

    #[async_trait]
    impl Storage for UnreadableStorage {
        async fn get(&self, _key: &StorageKey) -> Result<Bytes, StorageError> {
            Err(StorageError::GenericError("timeout".to_string()))
        }

        async fn get_with_meta(&self, key: &StorageKey) -> Result<ObjectWithMeta, StorageError> {
            self.0.get_with_meta(key).await
        }

        async fn put(&self, key: &StorageKey, object: Bytes) -> Result<(), StorageError> {
            self.0.put(key, object).await
        }

        async fn put_overwrite(&self, key: &StorageKey, object: Bytes) -> Result<(), StorageError> {
            self.0.put_overwrite(key, object).await
        }

        async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
            self.0.delete(key).await
        }

        async fn exists(&self, key: &StorageKey) -> Result<bool, StorageError> {
            self.0.exists(key).await
        }

        async fn size(&self, key: &StorageKey) -> Result<u64, StorageError> {
            self.0.size(key).await
        }

        async fn list(&self, prefix: &str) -> Result<Vec<StorageKey>, StorageError> {
            self.0.list(prefix).await
        }
    }

    #[tokio::test]
    async fn download_returns_500_if_the_storage_cannot_be_read() {
        let settings = get_settings();
        let storage = Box::new(UnreadableStorage(
            FSStorage::new(&settings.crates_path()).unwrap(),
        )) as DynStorage;
        let kellnr = TestKellnr::with_storage(settings, storage).await;
        let (status, detail) = publish_metadata(
            &kellnr,
            TOKEN,
            &PublishMetadata::minimal("unreadable", "1.0.0"),
        )
        .await;
        assert_eq!(StatusCode::OK, status, "{detail}");

        let r = kellnr
            .client
            .clone()
            .oneshot(
                Request::get("/api/v1/crates/unreadable/1.0.0/download")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, r.status());
    }

    #[tokio::test]
    async fn publish_completes_after_client_disconnect() {
        let valid_pub_package = read("../../tests/fixtures/test-data/pub_data.bin")
            .await
            .expect("Cannot open valid package file.");
        let settings = get_settings();
        let stored = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let storage = Box::new(GatedStorage {
            inner: FSStorage::new(&settings.crates_path()).unwrap(),
            stored: stored.clone(),
            release: release.clone(),
        }) as DynStorage;
        let kellnr = TestKellnr::with_storage(settings, storage).await;

        let request = tokio::spawn(
            kellnr.client.clone().oneshot(
                Request::put("/api/v1/crates/new")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, TOKEN)
                    .body(Body::from(valid_pub_package))
                    .unwrap(),
            ),
        );

        // Drop the handler future after the crate file is stored but before the DB
        // commit, like hyper does when the client disconnects.
        stored.notified().await;
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        release.notify_one();

        // The publish must still finish, otherwise the stored crate file is orphaned.
        tokio::time::timeout(Duration::from_secs(5), async {
            while test_utils::get_crate_meta_list(&kellnr.db, 1)
                .await
                .map_or(true, |m| m.is_empty())
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("Publish was not committed after the client disconnected");
    }

    struct TestKellnr {
        path: PathBuf,
        client: Router,
        db: Database,
    }

    fn get_settings() -> Settings {
        Settings {
            registry: kellnr_settings::Registry {
                data_dir: "/tmp/".to_string() + &generate_rand_string(10),
                session_age_seconds: 10,
                ..kellnr_settings::Registry::default()
            },
            ..Settings::default()
        }
    }

    fn generate_rand_string(length: usize) -> String {
        let mut rng = rng();
        iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(length)
            .collect::<String>()
    }

    impl TestKellnr {
        async fn new(settings: Settings) -> Self {
            std::fs::create_dir_all(&settings.registry.data_dir).unwrap();
            let con_string = ConString::Sqlite(SqliteConString::from(&settings));
            let db = Database::new(&con_string, 10).await.unwrap();
            TestKellnr {
                path: PathBuf::from(&settings.registry.data_dir),
                db,
                client: app(settings).await,
            }
        }

        async fn with_storage(settings: Settings, storage: DynStorage) -> Self {
            std::fs::create_dir_all(&settings.registry.data_dir).unwrap();
            let con_string = ConString::Sqlite(SqliteConString::from(&settings));
            let db = Database::new(&con_string, 10).await.unwrap();
            TestKellnr {
                path: PathBuf::from(&settings.registry.data_dir),
                db,
                client: Box::pin(app_with_storage(settings, storage)).await,
            }
        }

        async fn fake(settings: Settings) -> Self {
            std::fs::create_dir_all(&settings.registry.data_dir).unwrap();
            let con_string = ConString::Sqlite(SqliteConString::from(&settings));
            let db = Database::new(&con_string, 10).await.unwrap();

            TestKellnr {
                path: PathBuf::from(&settings.registry.data_dir),
                db,
                client: app(settings).await,
            }
        }
    }

    impl Drop for TestKellnr {
        fn drop(&mut self) {
            // SQLite can still remove its journal files while the directory is
            // removed, so a missing file is not an error here.
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    async fn app(settings: Settings) -> Router {
        let storage = Box::new(FSStorage::new(&settings.crates_path()).unwrap()) as DynStorage;
        Box::pin(app_with_storage(settings, storage)).await
    }

    async fn app_with_storage(settings: Settings, storage: DynStorage) -> Router {
        let con_string = ConString::Sqlite(SqliteConString::from(&settings));
        let db = Database::new(&con_string, 10).await.unwrap();
        let cs = KellnrCrateStorage::new(&settings, storage);
        db.add_auth_token("test", TOKEN, "admin").await.unwrap();
        db.add_user("ro_dummy", "ro", false, true).await.unwrap();
        db.add_auth_token("test ro", RO_TOKEN, "ro_dummy")
            .await
            .unwrap();
        db.add_user("ro_dummy_admin", "roa", true, true)
            .await
            .unwrap();
        db.add_auth_token("test admin ro", RO_ADMIN_TOKEN, "ro_dummy_admin")
            .await
            .unwrap();
        db.add_user("non_admin", "na", false, false).await.unwrap();
        db.add_auth_token("test non admin", NON_ADMIN_TOKEN, "non_admin")
            .await
            .unwrap();

        let db: Arc<dyn DbProvider> = Arc::new(db);
        let state = AppStateData {
            download_counter: Arc::new(DownloadCounter::new(db.clone(), 0)),
            db,
            download_filters: Arc::new(
                kellnr_filter::DownloadFilters::from_settings(&settings).unwrap(),
            ),
            settings: settings.into(),
            crate_storage: cs.into(),
            // Use a fixed signing key matching the test cookie helper so that
            // session cookies actually decrypt and session auth is exercised.
            signing_key: cookie::Key::from(kellnr_appstate::TEST_KEY),
            ..kellnr_appstate::test_state()
        };

        let routes = Router::new()
            .route("/{crate_name}/owners", delete(remove_owner))
            .route("/{crate_name}/owners", put(add_owner))
            .route("/{crate_name}/owners", get(list_owners))
            .route("/{crate_name}/owners/{user}", delete(remove_owner_single))
            .route("/{crate_name}/owners/{user}", put(add_owner_single))
            .route("/", get(search))
            .route("/{package}/{version}/download", get(download))
            .route("/new_empty", put(add_empty_crate))
            .route("/new", put(publish))
            .route("/{crate_name}/{version}/yank", delete(yank))
            .route("/{crate_name}/{version}/unyank", put(unyank));

        Router::new()
            .nest("/api/v1/crates", routes)
            .with_state(state)
    }

    fn app_search(db: Arc<dyn DbProvider>) -> Router {
        Router::new()
            .route("/api/v1/crates", get(search))
            .with_state(AppStateData {
                db,
                ..kellnr_appstate::test_state()
            })
    }
}
