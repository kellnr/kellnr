use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use kellnr_appstate::DbState;
use kellnr_db::{self, Group};
use serde::{Deserialize, Serialize};

use crate::error::RouteError;
use crate::session::AdminUser;

#[derive(Serialize)]
pub struct NewTokenResponse {
    name: String,
    token: String,
}

pub async fn list_groups(
    _user: AdminUser,
    State(db): DbState,
) -> Result<Json<Vec<Group>>, RouteError> {
    Ok(Json(db.get_groups().await?))
}

pub async fn delete(
    _user: AdminUser,
    Path(name): Path<String>,
    State(db): DbState,
) -> Result<(), RouteError> {
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
    _user: AdminUser,
    State(db): DbState,
    Json(new_group): Json<NewGroup>,
) -> Result<(), RouteError> {
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

pub async fn delete_user(
    _user: AdminUser,
    Path((group_name, name)): Path<(String, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    Ok(db.delete_group_user(&group_name, &name).await?)
}
