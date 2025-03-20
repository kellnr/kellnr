use crate::error::RouteError;
use crate::session::MaybeUser;
use appstate::DbState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use db::{self, Group};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct NewTokenResponse {
    name: String,
    token: String,
}

pub async fn list_groups(
    user: MaybeUser,
    State(db): DbState,
) -> Result<Json<Vec<Group>>, RouteError> {
    user.assert_admin()?;

    Ok(Json(db.get_groups().await?))
}

pub async fn delete(
    user: MaybeUser,
    Path(name): Path<String>,
    State(db): DbState,
) -> Result<(), RouteError> {
    user.assert_admin()?;

    Ok(db.delete_group(&name).await?)
}

#[derive(Deserialize)]
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

pub async fn add(
    user: MaybeUser,
    State(db): DbState,
    Json(new_group): Json<NewGroup>,
) -> Result<(), RouteError> {
    user.assert_admin()?;

    new_group.validate()?;

    Ok(db.add_group(&new_group.name).await?)
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupUser {
    pub id: i32,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupUserList {
    pub users: Vec<GroupUser>,
}

impl From<Vec<GroupUser>> for GroupUserList {
    fn from(users: Vec<GroupUser>) -> Self {
        Self { users }
    }
}

pub async fn list_users(
    user: MaybeUser,
    Path(group_name): Path<String>,
    State(db): DbState,
) -> Result<Json<GroupUserList>, RouteError> {
    user.assert_admin()?;

    let users: Vec<GroupUser> = db
        .get_group_users(&group_name)
        .await?
        .iter()
        .map(|u| GroupUser {
            id: u.id,
            name: u.name.to_owned(),
        })
        .collect();

    Ok(Json(GroupUserList::from(users)))
}

pub async fn add_user(
    user: MaybeUser,
    Path((group_name, name)): Path<(String, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    user.assert_admin()?;

    if !db.is_group_user(&group_name, &name).await? {
        db.add_group_user(&group_name, &name).await?;
    }

    Ok(())
}

pub async fn delete_user(
    user: MaybeUser,
    Path((group_name, name)): Path<(String, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    user.assert_admin()?;

    Ok(db.delete_group_user(&group_name, &name).await?)
}
