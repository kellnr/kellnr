use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use kellnr_appstate::DbState;
use kellnr_auth::maybe_user::MaybeUser;
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::original_name::OriginalName;
use kellnr_db::DbProvider;
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

enum AclAction {
    Read,
    Modify,
}

/// Admins and crate owners may manage the crate users and groups of a crate,
/// the same policy as the `/api/v1/crates/{crate_name}/crate_users` API.
/// Read-only owners may only read.
async fn authorize(
    user: &MaybeUser,
    crate_name: &CanonicalName,
    action: AclAction,
    db: &Arc<dyn DbProvider>,
) -> Result<(), RouteError> {
    if user.is_admin {
        return Ok(());
    }
    if matches!(action, AclAction::Modify) && user.is_read_only {
        return Err(RouteError::InsufficientPrivileges);
    }
    if db.is_owner(crate_name, &user.name).await? {
        Ok(())
    } else {
        Err(RouteError::InsufficientPrivileges)
    }
}

/// List users with access to a crate (admin or crate owner)
#[utoipa::path(
    get,
    path = "/{crate_name}/users",
    tag = "acl",
    params(
        ("crate_name" = OriginalName, Path, description = "Crate name")
    ),
    responses(
        (status = 200, description = "List of users with access", body = CrateUserList),
        (status = 403, description = "Admin or crate owner access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn list_users(
    user: MaybeUser,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> Result<Json<CrateUserList>, RouteError> {
    let crate_name = crate_name.to_canonical();
    authorize(&user, &crate_name, AclAction::Read, &db).await?;
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

/// Add a user to a crate's access list (admin or crate owner)
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
        (status = 403, description = "Admin or crate owner access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn add_user(
    user: MaybeUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    let crate_name = crate_name.to_canonical();
    authorize(&user, &crate_name, AclAction::Modify, &db).await?;

    // Fails with 404 for an unknown user.
    db.get_user(&name).await?;

    if !db.is_crate_user(&crate_name, &name).await? {
        db.add_crate_user(&crate_name, &name).await?;
    }

    Ok(())
}

/// Remove a user from a crate's access list (admin or crate owner)
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
        (status = 403, description = "Admin or crate owner access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_user(
    user: MaybeUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    let crate_name = crate_name.to_canonical();
    authorize(&user, &crate_name, AclAction::Modify, &db).await?;
    Ok(db.delete_crate_user(&crate_name, &name).await?)
}

/// List groups with access to a crate (admin or crate owner)
#[utoipa::path(
    get,
    path = "/{crate_name}/groups",
    tag = "acl",
    params(
        ("crate_name" = OriginalName, Path, description = "Crate name")
    ),
    responses(
        (status = 200, description = "List of groups with access", body = CrateGroupList),
        (status = 403, description = "Admin or crate owner access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn list_groups(
    user: MaybeUser,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> Result<Json<CrateGroupList>, RouteError> {
    let crate_name = crate_name.to_canonical();
    authorize(&user, &crate_name, AclAction::Read, &db).await?;
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

/// Add a group to a crate's access list (admin or crate owner)
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
        (status = 403, description = "Admin or crate owner access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn add_group(
    user: MaybeUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    let crate_name = crate_name.to_canonical();
    authorize(&user, &crate_name, AclAction::Modify, &db).await?;
    if !db.is_crate_group(&crate_name, &name).await? {
        db.add_crate_group(&crate_name, &name).await?;
    }

    Ok(())
}

/// Remove a group from a crate's access list (admin or crate owner)
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
        (status = 403, description = "Admin or crate owner access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn delete_group(
    user: MaybeUser,
    Path((crate_name, name)): Path<(OriginalName, String)>,
    State(db): DbState,
) -> Result<(), RouteError> {
    let crate_name = crate_name.to_canonical();
    authorize(&user, &crate_name, AclAction::Modify, &db).await?;
    Ok(db.delete_crate_group(&crate_name, &name).await?)
}

/// Get a crate's access settings (admin or crate owner)
#[utoipa::path(
    get,
    path = "/{crate_name}",
    tag = "acl",
    params(
        ("crate_name" = OriginalName, Path, description = "Crate name")
    ),
    responses(
        (status = 200, description = "Access settings", body = AccessData),
        (status = 403, description = "Admin or crate owner access required")
    ),
    security(("session_cookie" = []))
)]
pub async fn get_access_data(
    user: MaybeUser,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> Result<Json<AccessData>, RouteError> {
    let crate_name = crate_name.to_canonical();
    authorize(&user, &crate_name, AclAction::Read, &db).await?;
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
    let crate_name = crate_name.to_canonical();
    db.change_download_restricted(&crate_name, input.download_restricted)
        .await?;

    Ok(Json(AccessData {
        download_restricted: db.is_download_restricted(&crate_name).await?,
    }))
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum::routing::{get, put};
    use axum_extra::extract::cookie::Key;
    use kellnr_appstate::{AppStateData, TEST_KEY, encode_cookies};
    use kellnr_db::mock::MockDb;
    use kellnr_db::{SessionInfo, User};
    use kellnr_settings::constants::COOKIE_SESSION_ID;
    use mockall::predicate::eq;
    use tower::ServiceExt;

    use super::*;

    fn crate_name() -> CanonicalName {
        CanonicalName::try_from("mycrate").unwrap()
    }

    /// Mock with a session for `name` who owns "mycrate" if `is_owner`.
    fn session_db(
        name: &'static str,
        is_admin: bool,
        is_read_only: bool,
        is_owner: bool,
    ) -> MockDb {
        let mut db = MockDb::new();
        db.expect_validate_session().returning(move |_| {
            Ok(SessionInfo {
                name: name.to_string(),
                is_admin,
                is_read_only,
            })
        });
        db.expect_is_owner()
            .with(eq(crate_name()), eq(name))
            .returning(move |_, _| Ok(is_owner));
        db.expect_get_crate_users().returning(|_| Ok(vec![]));
        db.expect_get_crate_groups().returning(|_| Ok(vec![]));
        db.expect_get_user().returning(|name| {
            Ok(User {
                name: name.to_string(),
                ..User::default()
            })
        });
        db.expect_is_crate_user().returning(|_, _| Ok(false));
        db.expect_add_crate_user().returning(|_, _| Ok(()));
        db.expect_delete_crate_user().returning(|_, _| Ok(()));
        db.expect_is_crate_group().returning(|_, _| Ok(false));
        db.expect_add_crate_group().returning(|_, _| Ok(()));
        db.expect_delete_crate_group().returning(|_, _| Ok(()));
        db.expect_is_download_restricted().returning(|_| Ok(false));
        db.expect_change_download_restricted()
            .returning(|_, _| Ok(()));
        db
    }

    async fn send(db: MockDb, request: axum::http::request::Builder, body: Body) -> StatusCode {
        let app = Router::new()
            .route("/{crate_name}", get(get_access_data).put(set_access_data))
            .route("/{crate_name}/users", get(list_users))
            .route(
                "/{crate_name}/users/{name}",
                put(add_user).delete(delete_user),
            )
            .route("/{crate_name}/groups", get(list_groups))
            .route(
                "/{crate_name}/groups/{name}",
                put(add_group).delete(delete_group),
            )
            .with_state(AppStateData {
                db: Arc::new(db),
                signing_key: Key::from(TEST_KEY),
                ..kellnr_appstate::test_state()
            });
        let request = request
            .header(
                header::COOKIE,
                encode_cookies([(COOKIE_SESSION_ID, "cookie")]),
            )
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)
            .unwrap();
        app.oneshot(request).await.unwrap().status()
    }

    async fn status(db: MockDb, method: &str, uri: &str) -> StatusCode {
        send(
            db,
            Request::builder().method(method).uri(uri),
            Body::empty(),
        )
        .await
    }

    const ACL_REQUESTS: [(&str, &str); 7] = [
        ("GET", "/mycrate"),
        ("GET", "/mycrate/users"),
        ("PUT", "/mycrate/users/someone"),
        ("DELETE", "/mycrate/users/someone"),
        ("GET", "/mycrate/groups"),
        ("PUT", "/mycrate/groups/somegroup"),
        ("DELETE", "/mycrate/groups/somegroup"),
    ];

    #[tokio::test]
    async fn owner_can_manage_crate_users_and_groups() {
        for (method, uri) in ACL_REQUESTS {
            let db = session_db("owner", false, false, true);
            assert_eq!(
                StatusCode::OK,
                status(db, method, uri).await,
                "{method} {uri}"
            );
        }
    }

    #[tokio::test]
    async fn admin_can_manage_crate_users_and_groups() {
        for (method, uri) in ACL_REQUESTS {
            let db = session_db("admin", true, false, false);
            assert_eq!(
                StatusCode::OK,
                status(db, method, uri).await,
                "{method} {uri}"
            );
        }
    }

    #[tokio::test]
    async fn non_owner_is_forbidden() {
        for (method, uri) in ACL_REQUESTS {
            let db = session_db("user", false, false, false);
            assert_eq!(
                StatusCode::FORBIDDEN,
                status(db, method, uri).await,
                "{method} {uri}"
            );
        }
    }

    #[tokio::test]
    async fn read_only_owner_can_only_read() {
        for (method, uri) in ACL_REQUESTS {
            let db = session_db("owner", false, true, true);
            let expected = if method == "GET" {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            };
            assert_eq!(expected, status(db, method, uri).await, "{method} {uri}");
        }
    }

    #[tokio::test]
    async fn owner_cannot_change_download_restriction() {
        let db = session_db("owner", false, false, true);
        let body = Body::from(r#"{"download_restricted":true}"#);
        let status = send(db, Request::put("/mycrate"), body).await;
        assert_eq!(StatusCode::FORBIDDEN, status);
    }

    #[tokio::test]
    async fn admin_can_change_download_restriction() {
        let db = session_db("admin", true, false, false);
        let body = Body::from(r#"{"download_restricted":true}"#);
        let status = send(db, Request::put("/mycrate"), body).await;
        assert_eq!(StatusCode::OK, status);
    }
}
