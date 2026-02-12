use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use kellnr_appstate::DbState;
use kellnr_db::{self, Group};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::RouteError;
use crate::session::AdminUser;

#[derive(Serialize, ToSchema)]
pub struct NewTokenResponse {
    name: String,
    token: String,
}

/// List all groups (admin only)
#[utoipa::path(
    get,
    path = "/",
    tag = "groups",
    responses(
        (status = 200, description = "List of all groups", body = Vec<Group>),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn list_groups(
    _user: AdminUser,
    State(db): DbState,
) -> Result<Json<Vec<Group>>, RouteError> {
    Ok(Json(db.get_groups().await?))
}

/// Delete a group (admin only)
#[utoipa::path(
    delete,
    path = "/{name}",
    tag = "groups",
    params(
        ("name" = String, Path, description = "Group name to delete")
    ),
    responses(
        (status = 200, description = "Group deleted successfully"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete(
    _user: AdminUser,
    Path(name): Path<String>,
    State(db): DbState,
) -> Result<(), RouteError> {
    Ok(db.delete_group(&name).await?)
}

#[derive(Deserialize, ToSchema)]
pub struct NewGroup {
    pub name: String,
}

impl NewGroup {
    pub fn validate(&self) -> Result<(), RouteError> {
        if self.name.is_empty() {
            return Err(RouteError::Status(StatusCode::BAD_REQUEST));
        }
        Ok(())
    }
}

/// Create a new group (admin only)
#[utoipa::path(
    post,
    path = "/",
    tag = "groups",
    request_body = NewGroup,
    responses(
        (status = 200, description = "Group created successfully"),
        (status = 400, description = "Validation failed"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn add(
    _user: AdminUser,
    State(db): DbState,
    Json(new_group): Json<NewGroup>,
) -> Result<(), RouteError> {
    new_group.validate()?;

    Ok(db.add_group(&new_group.name).await?)
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GroupUser {
    pub id: i32,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GroupUserList {
    pub users: Vec<GroupUser>,
}

impl From<Vec<GroupUser>> for GroupUserList {
    fn from(users: Vec<GroupUser>) -> Self {
        Self { users }
    }
}

/// List members of a group (admin only)
#[utoipa::path(
    get,
    path = "/{group_name}/members",
    tag = "groups",
    params(
        ("group_name" = String, Path, description = "Group name")
    ),
    responses(
        (status = 200, description = "List of group members", body = GroupUserList),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn list_users(
    _user: AdminUser,
    Path(group_name): Path<String>,
    State(db): DbState,
) -> Result<Json<GroupUserList>, RouteError> {
    let users: Vec<GroupUser> = db
        .get_group_users(&group_name)
        .await?
        .iter()
        .map(|u| GroupUser {
            id: u.id,
            name: u.name.clone(),
        })
        .collect();

    Ok(Json(GroupUserList::from(users)))
}

/// Add a user to a group (admin only)
#[utoipa::path(
    put,
    path = "/{group_name}/members/{name}",
    tag = "groups",
    params(
        ("group_name" = String, Path, description = "Group name"),
        ("name" = String, Path, description = "Username to add")
    ),
    responses(
        (status = 200, description = "User added to group successfully"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn add_user(
    _user: AdminUser,
    Path((group_name, name)): Path<(String, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    if !db.is_group_user(&group_name, &name).await? {
        db.add_group_user(&group_name, &name).await?;
    }

    Ok(())
}

/// Remove a user from a group (admin only)
#[utoipa::path(
    delete,
    path = "/{group_name}/members/{name}",
    tag = "groups",
    params(
        ("group_name" = String, Path, description = "Group name"),
        ("name" = String, Path, description = "Username to remove")
    ),
    responses(
        (status = 200, description = "User removed from group successfully"),
        (status = 403, description = "Admin access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_user(
    _user: AdminUser,
    Path((group_name, name)): Path<(String, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    Ok(db.delete_group_user(&group_name, &name).await?)
}
