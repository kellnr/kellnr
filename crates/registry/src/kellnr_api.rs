use std::convert::TryFrom;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::Utc;
use kellnr_appstate::{AppState, DbState};
use kellnr_auth::{maybe_user, token};
use kellnr_common::normalized_name::NormalizedName;
use kellnr_common::original_name::OriginalName;
use kellnr_common::search_result;
use kellnr_common::search_result::{Crate, SearchResult};
use kellnr_common::version::Version;
use kellnr_common::webhook::WebhookEvent;
use kellnr_db::DbProvider;
use kellnr_error::api_error::{ApiError, ApiResult};
use tracing::warn;

use crate::pub_data::{EmptyCrateData, PubData};
use crate::pub_success::{EmptyCrateSuccess, PubDataSuccess};
use crate::registry_error::RegistryError;
use crate::search_params::SearchParams;

#[cfg(test)]
#[path = "test_cookie_helper.rs"]
mod test_cookie_helper;

use crate::yank_success::YankSuccess;
use crate::{crate_group, crate_user, crate_version};

pub async fn check_ownership(
    crate_name: &NormalizedName,
    user: &maybe_user::MaybeUser,
    db: &Arc<dyn DbProvider>,
) -> Result<(), ApiError> {
    if user.is_admin || db.is_owner(crate_name, &user.name).await? {
        Ok(())
    } else {
        Err(RegistryError::NotOwner.into())
    }
}

pub fn check_can_modify(user: &maybe_user::MaybeUser) -> Result<(), ApiError> {
    if !user.is_admin && user.is_read_only {
        Err(RegistryError::ReadOnlyModify.into())
    } else {
        Ok(())
    }
}

pub async fn check_download_auth(
    crate_name: &NormalizedName,
    token: &token::OptionToken,
    db: &Arc<dyn DbProvider>,
) -> ApiResult<()> {
    if !db.is_download_restricted(crate_name).await? {
        return Ok(());
    }

    let token = match token {
        token::OptionToken::Some(token) => token,
        token::OptionToken::None => return Err(RegistryError::DownloadUnauthorized.into()),
    };

    if token.is_admin
        || db.is_crate_user(crate_name, &token.user).await?
        || db.is_crate_group_user(crate_name, &token.user).await?
        || db.is_owner(crate_name, &token.user).await?
    {
        Ok(())
    } else {
        Err(RegistryError::NotCrateUser.into())
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

    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &user, db).await?;

    if !settings.registry.allow_ownerless_crates {
        // Never allow removing the last owner.
        let owners = db.get_crate_owners(&crate_name).await?;
        let owners_to_remove = input.users.len();
        if owners.len().saturating_sub(owners_to_remove) == 0 {
            return Err(RegistryError::LastOwner.into());
        }
    }

    for user in &input.users {
        db.delete_owner(&crate_name, user).await?;
    }

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

    let crate_name = crate_name.to_normalized();
    check_ownership(&crate_name, &user, db).await?;

    if !settings.registry.allow_ownerless_crates {
        // Never allow removing the last owner.
        let owners = db.get_crate_owners(&crate_name).await?;
        if owners.len() <= 1 {
            return Err(RegistryError::LastOwner.into());
        }
    }

    db.delete_owner(&crate_name, &removed_user).await?;

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

    let crate_name = crate_name.to_normalized();
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
    let crate_name = crate_name.to_normalized();
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

    let crate_name = crate_name.to_normalized();
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

    let crate_name = crate_name.to_normalized();
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
        (status = 200, description = "List of owners", body = crate_user::CrateUserList)
    ),
    security(("cargo_token" = []))
)]
pub async fn list_owners(
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> ApiResult<Json<crate_user::CrateUserList>> {
    let crate_name = crate_name.to_normalized();

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

    let crate_name = crate_name.to_normalized();
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
    let crate_name = crate_name.to_normalized();
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
    let crate_name = crate_name.to_normalized();
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
    let crate_name = crate_name.to_normalized();
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
        (status = 200, description = "List of versions", body = crate_version::CrateVersionList)
    ),
    security(("cargo_token" = []))
)]
pub async fn list_crate_versions(
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> ApiResult<Json<crate_version::CrateVersionList>> {
    let crate_name = crate_name.to_normalized();

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
/// Searches for crates by name in the Kellnr registry.
#[utoipa::path(
    get,
    path = "/",
    tag = "crates",
    params(
        ("q" = String, Query, description = "Search query"),
        ("per_page" = Option<u32>, Query, description = "Results per page")
    ),
    responses(
        (status = 200, description = "Search results", body = SearchResult)
    ),
    security(("cargo_token" = []))
)]
pub async fn search(State(db): DbState, params: SearchParams) -> ApiResult<Json<SearchResult>> {
    let crates = db
        .search_in_crate_name(&params.q, false)
        .await?
        .into_iter()
        .map(|c| Crate {
            name: c.name,
            max_version: c.version,
            description: c
                .description
                .unwrap_or_else(|| "No description set".to_string()),
        })
        .take(params.per_page.0)
        .collect::<Vec<Crate>>();

    Ok(Json(SearchResult {
        meta: search_result::Meta {
            total: crates.len() as i32,
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
        (status = 404, description = "Crate not found"),
        (status = 401, description = "Unauthorized for restricted crate")
    ),
    security(("cargo_token" = []))
)]
pub async fn download(
    State(state): AppState,
    token: token::OptionToken,
    Path((package, version)): Path<(OriginalName, Version)>,
) -> ApiResult<Vec<u8>> {
    let db = state.db;
    let cs = state.crate_storage;
    check_download_auth(&package.to_normalized(), &token, &db).await?;

    if let Err(e) = db
        .increase_download_counter(&package.to_normalized(), &version)
        .await
    {
        warn!("Failed to increase download counter: {e}");
    }

    match cs.get(&package, &version).await {
        Some(file) => Ok(file),
        None => Err(RegistryError::CrateNotFound.into()),
    }
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
        (status = 400, description = "Crate already exists"),
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
        return Err(ApiError::new("Unauthorized", "", StatusCode::UNAUTHORIZED));
    }
    let db = state.db;
    let orig_name = OriginalName::try_from(&data.name)?;
    let normalized_name = orig_name.to_normalized();

    if let Some(id) = db.get_crate_id(&normalized_name).await? {
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
        (status = 400, description = "Invalid crate data or version exists"),
        (status = 403, description = "Not an owner")
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
    let normalized_name = orig_name.to_normalized();

    let token_user = token.user.clone();
    let user = maybe_user::MaybeUser::from_token(token);

    // Check if user is read-only and can't publish (upload) crates.
    // Admin users bypass this check as they can modify
    // their read-only status.
    check_can_modify(&user)?;

    // Check if user from token is an owner of the crate.
    // If not, he is not allowed push a new version.
    // Check if crate with same version already exists.
    let id = db.get_crate_id(&normalized_name).await?;
    match (id, settings.registry.new_crates_restricted) {
        (Some(id), _) => {
            check_ownership(&normalized_name, &user, &db).await?;

            if db.crate_version_exists(id, &pub_data.metadata.vers).await? {
                return Err(RegistryError::CrateExists(
                    pub_data.metadata.name,
                    pub_data.metadata.vers,
                )
                .into());
            }
        }
        (None, true) => {
            if !user.is_admin {
                return Err(RegistryError::NewCratesRestricted.into());
            }
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

    // Set SHA256 from crate file
    let version = Version::try_from(&pub_data.metadata.vers)?;
    let cksum = cs
        .put(&orig_name, &version, pub_data.cratedata.clone())
        .await?;

    let created = Utc::now();

    // Add crate to DB
    if let Err(e) = db
        .add_crate(&pub_data.metadata, &cksum, &created, &token_user)
        .await
    {
        // On DB error rollback storage insert and bail.
        let _ = cs.delete(&orig_name, &version).await;
        return Err(e.into());
    }

    kellnr_webhooks::notify_crate(
        if id.is_none() {
            WebhookEvent::CrateAdd
        } else {
            WebhookEvent::CrateUpdate
        },
        &created,
        &normalized_name,
        &version,
        &db,
    )
    .await;

    // Add crate to queue for doc extraction if there is no documentation value set already
    if settings.docs.enabled && pub_data.metadata.documentation.is_none() {
        db.add_doc_queue(
            &normalized_name,
            &version,
            &cs.create_rand_doc_queue_path().await?,
        )
        .await?;
    }

    Ok(Json(PubDataSuccess::new()))
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

    let crate_name = crate_name.to_normalized();
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

    let crate_name = crate_name.to_normalized();
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

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::{delete, get, put};
    use http_body_util::BodyExt;
    use hyper::header;
    use kellnr_appstate::AppStateData;
    use kellnr_db::mock::MockDb;
    use kellnr_db::{ConString, Database, SqliteConString, test_utils};
    use kellnr_error::api_error::ErrorDetails;
    use kellnr_settings::Settings;
    use kellnr_storage::cached_crate_storage::DynStorage;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
    use mockall::predicate::*;
    use rand::distr::Alphanumeric;
    use rand::{Rng, rng};
    use tokio::fs::read;
    use tower::ServiceExt;

    use super::*;

    const TOKEN: &str = "854DvwSlUwEHtIo3kWy6x7UCPKHfzCmy";
    const NON_ADMIN_TOKEN: &str = "g8kfzxSrMswNOVio5kBoTEFBBVm3fRS7";
    const RO_TOKEN: &str = "lJh6orU1Ye376ApXJR8I7V9gI3V6UZWU";
    const RO_ADMIN_TOKEN: &str = "GUOMPlZwN1kliXRW5wJ0ixh54NqYlE6X";

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
                .get_crate_owners(&NormalizedName::from_unchecked("test_lib".to_string()))
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
                .get_crate_owners(&NormalizedName::from_unchecked("test_lib".to_string()))
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
            .add_user("user", "123", "123", false, false)
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
            .add_user("user", "123", "123", false, false)
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
            .get_crate_owners(&NormalizedName::from_unchecked("test_lib".to_string()))
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
            .get_crate_owners(&NormalizedName::from_unchecked("test_lib".to_string()))
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
            .add_user("user", "123", "123", false, false)
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
            .add_user("user", "123", "123", false, false)
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
            .add_user("user", "123", "123", false, false)
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
            .add_user("user", "123", "123", false, false)
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
            .add_user("user", "123", "123", false, false)
            .await
            .unwrap();

        // create a session token and store it in DB
        kellnr.db.add_session_token("user", "1234").await.unwrap();

        // create signed cookie header
        let cookie_header = test_cookie_helper::cookies::encode_cookies([(
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

        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
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
            .add_user("user", "123", "123", false, false)
            .await
            .unwrap();

        // create a session token and store it in DB
        kellnr.db.add_session_token("user", "1234").await.unwrap();

        let cookie_header = test_cookie_helper::cookies::encode_cookies([(
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

        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
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

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn search_verify_query_and_default() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_search_in_crate_name()
            .with(eq("foo"), eq(false))
            .returning(|_, _| Ok(vec![]));

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
    async fn search_verify_per_page() {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_search_in_crate_name()
            .with(eq("foo"), eq(false))
            .returning(|_, _| Ok(vec![]));

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

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert!(serde_json::from_slice::<SearchResult>(&result_msg).is_err());
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
        let normalized_name = OriginalName::try_from("next-crate")
            .unwrap()
            .to_normalized();
        let crate_id = kellnr
            .db
            .get_crate_id(&normalized_name)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            Version::from_unchecked_str("0.0.0"),
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

        let normalized_name = OriginalName::try_from("rogue-crate")
            .unwrap()
            .to_normalized();
        assert_eq!(
            None,
            kellnr.db.get_crate_id(&normalized_name).await.unwrap()
        );
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

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
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
        let normalized_name = OriginalName::try_from("test_lib").unwrap().to_normalized();
        kellnr
            .db
            .add_owner(&normalized_name, "non_admin")
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
                .get_crate_meta_list(&normalized_name)
                .await
                .unwrap()
                .len()
        );
        assert_eq!(
            "0.2.0",
            kellnr
                .db
                .get_crate_meta_list(&normalized_name)
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

        assert_eq!(StatusCode::BAD_REQUEST, response_status);
        assert_eq!(
            "ERROR: Crate with version already exists: test_lib-0.2.0",
            error.errors[0].detail
        );
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
            rm_rf::remove(&self.path).expect("Cannot remove TestKellnr");
        }
    }

    async fn app(settings: Settings) -> Router {
        let con_string = ConString::Sqlite(SqliteConString::from(&settings));
        let db = Database::new(&con_string, 10).await.unwrap();
        let storage = Box::new(FSStorage::new(&settings.crates_path()).unwrap()) as DynStorage;
        let cs = KellnrCrateStorage::new(&settings, storage);
        db.add_auth_token("test", TOKEN, "admin").await.unwrap();
        db.add_user("ro_dummy", "ro", "", false, true)
            .await
            .unwrap();
        db.add_auth_token("test ro", RO_TOKEN, "ro_dummy")
            .await
            .unwrap();
        db.add_user("ro_dummy_admin", "roa", "", true, true)
            .await
            .unwrap();
        db.add_auth_token("test admin ro", RO_ADMIN_TOKEN, "ro_dummy_admin")
            .await
            .unwrap();
        db.add_user("non_admin", "na", "", false, false)
            .await
            .unwrap();
        db.add_auth_token("test non admin", NON_ADMIN_TOKEN, "non_admin")
            .await
            .unwrap();

        let state = AppStateData {
            db: Arc::new(db),
            settings: settings.into(),
            crate_storage: cs.into(),
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
