use axum::Json;
use axum::extract::{Path, State};
use kellnr_appstate::DbState;
use kellnr_common::original_name::OriginalName;
use kellnr_registry::crate_group::{CrateGroup, CrateGroupList};
use kellnr_registry::crate_user::{CrateUser, CrateUserList};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::RouteError;
use crate::session::AdminUser;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AccessData {
    pub download_restricted: bool,
}

/// List users with access to a crate (admin only)
#[utoipa::path(
    get,
    path = "/{crate_name}/users",
    tag = "acl",
    params(
        ("crate_name" = OriginalName, Path, description = "Crate name")
    ),
    responses(
        (status = 200, description = "List of users with access", body = CrateUserList),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn list_users(
    _user: AdminUser,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> Result<Json<CrateUserList>, RouteError> {
    let crate_name = crate_name.to_normalized();
    let users: Vec<CrateUser> = db
        .get_crate_users(&crate_name)
        .await?
        .iter()
        .map(|u| CrateUser {
            id: u.id,
            login: u.name.clone(),
            name: None,
        })
        .collect();

    Ok(Json(CrateUserList::from(users)))
}

/// Add a user to a crate's access list (admin only)
#[utoipa::path(
    put,
    path = "/{crate_name}/users/{name}",
    tag = "acl",
    params(
        ("crate_name" = OriginalName, Path, description = "Crate name"),
        ("name" = String, Path, description = "Username to add")
    ),
    responses(
        (status = 200, description = "User added successfully"),
        (status = 404, description = "User not found"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn add_user(
    _user: AdminUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    let crate_name = crate_name.to_normalized();

    if db.get_user(&name).await.is_err() {
        return Err(RouteError::UserNotFound(name));
    }

    if !db.is_crate_user(&crate_name, &name).await? {
        db.add_crate_user(&crate_name, &name).await?;
    }

    Ok(())
}

/// Remove a user from a crate's access list (admin only)
#[utoipa::path(
    delete,
    path = "/{crate_name}/users/{name}",
    tag = "acl",
    params(
        ("crate_name" = OriginalName, Path, description = "Crate name"),
        ("name" = String, Path, description = "Username to remove")
    ),
    responses(
        (status = 200, description = "User removed successfully"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_user(
    _user: AdminUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    let crate_name = crate_name.to_normalized();
    Ok(db.delete_crate_user(&crate_name, &name).await?)
}

/// List groups with access to a crate (admin only)
#[utoipa::path(
    get,
    path = "/{crate_name}/groups",
    tag = "acl",
    params(
        ("crate_name" = OriginalName, Path, description = "Crate name")
    ),
    responses(
        (status = 200, description = "List of groups with access", body = CrateGroupList),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn list_groups(
    _user: AdminUser,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> Result<Json<CrateGroupList>, RouteError> {
    let crate_name = crate_name.to_normalized();
    let groups: Vec<CrateGroup> = db
        .get_crate_groups(&crate_name)
        .await?
        .iter()
        .map(|u| CrateGroup {
            id: u.id,
            name: u.name.clone(),
        })
        .collect();

    Ok(Json(CrateGroupList::from(groups)))
}

/// Add a group to a crate's access list (admin only)
#[utoipa::path(
    put,
    path = "/{crate_name}/groups/{name}",
    tag = "acl",
    params(
        ("crate_name" = OriginalName, Path, description = "Crate name"),
        ("name" = String, Path, description = "Group name to add")
    ),
    responses(
        (status = 200, description = "Group added successfully"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn add_group(
    _user: AdminUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    let crate_name = crate_name.to_normalized();
    if !db.is_crate_group(&crate_name, &name).await? {
        db.add_crate_group(&crate_name, &name).await?;
    }

    Ok(())
}

/// Remove a group from a crate's access list (admin only)
#[utoipa::path(
    delete,
    path = "/{crate_name}/groups/{name}",
    tag = "acl",
    params(
        ("crate_name" = OriginalName, Path, description = "Crate name"),
        ("name" = String, Path, description = "Group name to remove")
    ),
    responses(
        (status = 200, description = "Group removed successfully"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_group(
    _user: AdminUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    let crate_name = crate_name.to_normalized();
    Ok(db.delete_crate_group(&crate_name, &name).await?)
}

/// Get a crate's access settings (admin only)
#[utoipa::path(
    get,
    path = "/{crate_name}",
    tag = "acl",
    params(
        ("crate_name" = OriginalName, Path, description = "Crate name")
    ),
    responses(
        (status = 200, description = "Access settings", body = AccessData),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn get_access_data(
    _user: AdminUser,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> Result<Json<AccessData>, RouteError> {
    let crate_name = crate_name.to_normalized();
    Ok(Json(AccessData {
        download_restricted: db.is_download_restricted(&crate_name).await?,
    }))
}

/// Update a crate's access settings (admin only)
#[utoipa::path(
    put,
    path = "/{crate_name}",
    tag = "acl",
    params(
        ("crate_name" = OriginalName, Path, description = "Crate name")
    ),
    request_body = AccessData,
    responses(
        (status = 200, description = "Access settings updated", body = AccessData),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn set_access_data(
    _user: AdminUser,
    State(db): DbState,
    Path(crate_name): Path<OriginalName>,
    Json(input): Json<AccessData>,
) -> Result<Json<AccessData>, RouteError> {
    let crate_name = crate_name.to_normalized();
    db.change_download_restricted(&crate_name, input.download_restricted)
        .await?;

    Ok(Json(AccessData {
        download_restricted: db.is_download_restricted(&crate_name).await?,
    }))
}
