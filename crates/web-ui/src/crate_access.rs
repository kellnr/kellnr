use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use kellnr_http::state::DbState;
use kellnr_http::maybe_user::MaybeUser;
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::original_name::OriginalName;
use kellnr_db::DbProvider;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::RouteError;
use crate::session::AdminUser;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AccessData {
    pub download_restricted: bool,
}

/// Admins and crate owners, also read-only ones, may read the access settings
/// of a crate.
async fn authorize_read(
    user: &MaybeUser,
    crate_name: &CanonicalName,
    db: &Arc<dyn DbProvider>,
) -> Result<(), RouteError> {
    if user.is_admin || db.is_owner(crate_name, &user.name).await? {
        Ok(())
    } else {
        Err(RouteError::InsufficientPrivileges)
    }
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
    authorize_read(&user, &crate_name, &db).await?;
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
    use axum::routing::get;
    use axum_extra::extract::cookie::Key;
    use kellnr_http::state::AppStateData;
    use kellnr_http::test_support::{TEST_KEY, encode_cookies};
    use kellnr_common::account_name::UserName;
    use kellnr_db::SessionInfo;
    use kellnr_db::mock::MockDb;
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
                name: UserName::new(name).unwrap(),
                is_admin,
                is_read_only,
            })
        });
        db.expect_is_owner()
            .with(eq(crate_name()), eq(UserName::new(name).unwrap()))
            .returning(move |_, _| Ok(is_owner));
        db.expect_is_download_restricted().returning(|_| Ok(false));
        db.expect_change_download_restricted()
            .returning(|_, _| Ok(()));
        db
    }

    async fn send(db: MockDb, request: axum::http::request::Builder, body: Body) -> StatusCode {
        let app = Router::new()
            .route("/{crate_name}", get(get_access_data).put(set_access_data))
            .with_state(AppStateData {
                db: Arc::new(db),
                signing_key: Key::from(TEST_KEY),
                ..kellnr_http::test_support::test_state()
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

    #[tokio::test]
    async fn owner_and_admin_can_read_access_settings() {
        for (name, is_admin, is_read_only, is_owner) in [
            ("owner", false, false, true),
            ("owner", false, true, true),
            ("admin", true, false, false),
        ] {
            let db = session_db(name, is_admin, is_read_only, is_owner);
            assert_eq!(
                StatusCode::OK,
                status(db, "GET", "/mycrate").await,
                "{name} read-only: {is_read_only}"
            );
        }
    }

    #[tokio::test]
    async fn non_owner_cannot_read_access_settings() {
        let db = session_db("user", false, false, false);
        assert_eq!(StatusCode::FORBIDDEN, status(db, "GET", "/mycrate").await);
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
