use crate::error::RouteError;
use crate::session::MaybeUser;
use appstate::DbState;
use axum::extract::{Path, State};
use axum::Json;
use common::original_name::OriginalName;
use registry::crate_user::{CrateUser, CrateUserList};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessData {
    pub download_restricted: bool,
}

pub async fn list_users(
    user: MaybeUser,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> Result<Json<CrateUserList>, RouteError> {
    user.assert_admin()?;

    let crate_name = crate_name.to_normalized();
    let users: Vec<CrateUser> = db
        .get_crate_users(&crate_name)
        .await?
        .iter()
        .map(|u| CrateUser {
            id: u.id,
            login: u.name.to_owned(),
            name: None,
        })
        .collect();

    Ok(Json(CrateUserList::from(users)))
}

pub async fn add_user(
    user: MaybeUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    user.assert_admin()?;

    let crate_name = crate_name.to_normalized();
    if !db.is_crate_user(&crate_name, &name).await? {
        db.add_crate_user(&crate_name, &name).await?;
    }

    Ok(())
}

pub async fn delete_user(
    user: MaybeUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    user.assert_admin()?;

    let crate_name = crate_name.to_normalized();
    Ok(db.delete_crate_user(&crate_name, &name).await?)
}

pub async fn get_access_data(
    user: MaybeUser,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> Result<Json<AccessData>, RouteError> {
    user.assert_admin()?;

    let crate_name = crate_name.to_normalized();
    Ok(Json(AccessData {
        download_restricted: db.is_download_restricted(&crate_name).await?,
    }))
}

pub async fn set_access_data(
    user: MaybeUser,
    State(db): DbState,
    Path(crate_name): Path<OriginalName>,
    Json(input): Json<AccessData>,
) -> Result<(), RouteError> {
    user.assert_admin()?;

    let crate_name = crate_name.to_normalized();
    db.change_download_restricted(&crate_name, input.download_restricted)
        .await?;

    Ok(())
}
