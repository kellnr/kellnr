use axum::Json;
use axum::extract::{Path, State};
use kellnr_common::account_name::{GroupName, UserName};
use kellnr_db::{self, Group, UserId};
use kellnr_http::error::ApiError;
use kellnr_http::identity::AdminUser;
use kellnr_http::state::DbState;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
) -> Result<Json<Vec<Group>>, ApiError> {
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
    Path(name): Path<GroupName>,
    State(db): DbState,
) -> Result<(), ApiError> {
    Ok(db.delete_group(&name).await?)
}

#[derive(Deserialize, ToSchema)]
pub struct NewGroup {
    pub name: String,
}

impl NewGroup {
    /// Returns the name of the new group.
    pub fn validate(&self) -> Result<GroupName, ApiError> {
        Ok(GroupName::for_new_group(self.name.as_str())?)
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
) -> Result<(), ApiError> {
    let name = new_group.validate()?;

    Ok(db.add_group(&name).await?)
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GroupUser {
    pub id: UserId,
    pub name: UserName,
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
    Path(group_name): Path<GroupName>,
    State(db): DbState,
) -> Result<Json<GroupUserList>, ApiError> {
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
    Path((group_name, name)): Path<(GroupName, UserName)>,
    State(db): DbState,
) -> Result<(), ApiError> {
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
    Path((group_name, name)): Path<(GroupName, UserName)>,
    State(db): DbState,
) -> Result<(), ApiError> {
    Ok(db.delete_group_user(&group_name, &name).await?)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::http::StatusCode;
    use kellnr_db::mock::MockDb;

    use super::*;

    #[tokio::test]
    async fn add_group_with_invalid_name_is_bad_request() {
        for name in ["", " ", "dev\tteam", &"a".repeat(65)] {
            let result = add(
                AdminUser(kellnr_http::test_support::session_identity("admin", true)),
                State(Arc::new(MockDb::new())),
                Json(NewGroup {
                    name: name.to_string(),
                }),
            )
            .await;

            assert!(
                result
                    .as_ref()
                    .is_err_and(|e| e.status() == StatusCode::BAD_REQUEST),
                "{name:?}"
            );
        }
    }
}
