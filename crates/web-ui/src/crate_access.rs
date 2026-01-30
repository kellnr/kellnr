use axum::Json;
use axum::extract::{Path, State};
use kellnr_appstate::DbState;
use kellnr_common::original_name::OriginalName;
use kellnr_registry::crate_group::{CrateGroup, CrateGroupList};
use kellnr_registry::crate_user::{CrateUser, CrateUserList};
use serde::{Deserialize, Serialize};

use crate::error::RouteError;
use crate::session::AdminUser;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessData {
    pub download_restricted: bool,
}

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

pub async fn delete_user(
    _user: AdminUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    let crate_name = crate_name.to_normalized();
    Ok(db.delete_crate_user(&crate_name, &name).await?)
}

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

pub async fn delete_group(
    _user: AdminUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    let crate_name = crate_name.to_normalized();
    Ok(db.delete_crate_group(&crate_name, &name).await?)
}

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
