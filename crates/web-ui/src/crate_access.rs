use axum::Json;
use axum::extract::{Path, State};
use kellnr_common::original_name::OriginalName;
use kellnr_http::authz;
use kellnr_http::error::ApiError;
use kellnr_http::identity::{AdminUser, AnyUser};
use kellnr_http::state::DbState;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AccessData {
    pub download_restricted: bool,
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
    user: AnyUser,
    Path(crate_name): Path<OriginalName>,
    State(db): DbState,
) -> Result<Json<AccessData>, ApiError> {
    let crate_name = crate_name.to_canonical();
    // Admins and crate owners, also read-only ones, may read the access
    // settings of a crate.
    authz::require_owner(&crate_name, &user, &db).await?;
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
) -> Result<Json<AccessData>, ApiError> {
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
    use kellnr_common::account_name::UserName;
    use kellnr_common::canonical_name::CanonicalName;
    use kellnr_db::test_utils::TestCrateBuilder;
    use kellnr_db::{DbProvider, UserFlags};
    use kellnr_http::state::AppStateData;
    use kellnr_http::test_support::{session_cookie, sqlite_test_state};
    use kellnr_test_db::TestDb;
    use tower::ServiceExt;

    use super::*;

    fn crate_name() -> CanonicalName {
        CanonicalName::try_from("mycrate").unwrap()
    }

    /// A database with "mycrate" owned by "owner", and a session cookie for
    /// `name` with `flags`.
    async fn state_with_session(name: &str, flags: UserFlags) -> (AppStateData, TestDb, String) {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, name, flags).await;
        if name != "owner" {
            db.add_user(&UserName::new("owner").unwrap(), "pwd", UserFlags::normal())
                .await
                .unwrap();
        }
        TestCrateBuilder::new(&db)
            .name("mycrate")
            .owner("owner")
            .build()
            .await
            .unwrap();
        (state, db, cookie)
    }

    async fn send(
        state: AppStateData,
        cookie: &str,
        request: axum::http::request::Builder,
        body: Body,
    ) -> StatusCode {
        let app = Router::new()
            .route("/{crate_name}", get(get_access_data).put(set_access_data))
            .with_state(state);
        let request = request
            .header(header::COOKIE, cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)
            .unwrap();
        app.oneshot(request).await.unwrap().status()
    }

    async fn restrict(state: AppStateData, cookie: &str) -> StatusCode {
        let body = Body::from(r#"{"download_restricted":true}"#);
        send(state, cookie, Request::put("/mycrate"), body).await
    }

    #[tokio::test]
    async fn owner_and_admin_can_read_access_settings() {
        for (name, flags) in [
            ("owner", UserFlags::normal()),
            ("owner", UserFlags::read_only()),
            ("admin", UserFlags::admin()),
        ] {
            let (state, _db, cookie) = state_with_session(name, flags).await;
            assert_eq!(
                StatusCode::OK,
                send(state, &cookie, Request::get("/mycrate"), Body::empty()).await,
                "{name} {flags:?}"
            );
        }
    }

    #[tokio::test]
    async fn non_owner_cannot_read_access_settings() {
        let (state, _db, cookie) = state_with_session("user", UserFlags::normal()).await;
        assert_eq!(
            StatusCode::FORBIDDEN,
            send(state, &cookie, Request::get("/mycrate"), Body::empty()).await
        );
    }

    #[tokio::test]
    async fn owner_cannot_change_download_restriction() {
        let (state, db, cookie) = state_with_session("owner", UserFlags::normal()).await;

        assert_eq!(StatusCode::FORBIDDEN, restrict(state, &cookie).await);
        assert!(!db.is_download_restricted(&crate_name()).await.unwrap());
    }

    #[tokio::test]
    async fn admin_can_change_download_restriction() {
        let (state, db, cookie) = state_with_session("admin", UserFlags::admin()).await;

        assert_eq!(StatusCode::OK, restrict(state, &cookie).await);
        assert!(db.is_download_restricted(&crate_name()).await.unwrap());
    }
}
