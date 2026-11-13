use std::sync::Arc;

use axum::extract::{Path, State};
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_db::DbProvider;
use kellnr_http::error::ApiError;
use kellnr_http::identity::{Identity, SessionUser};
use kellnr_http::state::DbState;
use kellnr_registry::yank::{YankAction, change_yanked};

/// Yank a crate version
///
/// Marks a crate version as yanked, so that cargo no longer selects it for
/// new dependencies. Requires ownership of the crate without read-only
/// restriction or admin access.
#[utoipa::path(
    put,
    path = "/crates/{name}/{version}/yank",
    tag = "ui",
    params(
        ("name" = String, Path, description = "Crate name"),
        ("version" = String, Path, description = "Version to yank")
    ),
    responses(
        (status = 200, description = "Version yanked"),
        (status = 400, description = "Invalid crate name or version"),
        (status = 401, description = "Not logged in"),
        (status = 403, description = "Not an owner of the crate or read-only"),
        (status = 404, description = "Version does not exist")
    ),
    security(("session_cookie" = []))
)]
pub async fn yank_crate_version(
    Path((name, version)): Path<(String, String)>,
    user: SessionUser,
    State(db): DbState,
) -> Result<(), ApiError> {
    change_yanked_impl(YankAction::Yank, &name, &version, &user, &db).await
}

/// Unyank a crate version
///
/// Removes the yanked status from a crate version. Requires ownership of the
/// crate without read-only restriction or admin access.
#[utoipa::path(
    put,
    path = "/crates/{name}/{version}/unyank",
    tag = "ui",
    params(
        ("name" = String, Path, description = "Crate name"),
        ("version" = String, Path, description = "Version to unyank")
    ),
    responses(
        (status = 200, description = "Version unyanked"),
        (status = 400, description = "Invalid crate name or version"),
        (status = 401, description = "Not logged in"),
        (status = 403, description = "Not an owner of the crate or read-only"),
        (status = 404, description = "Version does not exist")
    ),
    security(("session_cookie" = []))
)]
pub async fn unyank_crate_version(
    Path((name, version)): Path<(String, String)>,
    user: SessionUser,
    State(db): DbState,
) -> Result<(), ApiError> {
    change_yanked_impl(YankAction::Unyank, &name, &version, &user, &db).await
}

async fn change_yanked_impl(
    action: YankAction,
    name: &str,
    version: &str,
    user: &Identity,
    db: &Arc<dyn DbProvider>,
) -> Result<(), ApiError> {
    let name = OriginalName::try_from(name)?.to_canonical();
    let version = Version::try_from(version)?;
    change_yanked(action, &name, &version, user, db).await
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, Response, StatusCode, header};
    use axum::routing::put;
    use http_body_util::BodyExt;
    use kellnr_common::canonical_name::CanonicalName;
    use kellnr_common::webhook::{Webhook, WebhookEvent};
    use kellnr_db::UserFlags;
    use kellnr_db::test_utils::TestCrateBuilder;
    use kellnr_http::state::AppStateData;
    use kellnr_http::test_support::{session_cookie, sqlite_test_state};
    use kellnr_test_db::TestDb;
    use tower::ServiceExt;

    use super::*;

    fn app(state: AppStateData) -> Router {
        Router::new()
            .route("/crates/{name}/{version}/yank", put(yank_crate_version))
            .route("/crates/{name}/{version}/unyank", put(unyank_crate_version))
            .with_state(state)
    }

    /// Sends a PUT request, with a session if `cookie` is not empty.
    async fn put_with(state: AppStateData, uri: &str, cookie: &str) -> Response<Body> {
        let request = Request::put(uri);
        let request = if cookie.is_empty() {
            request
        } else {
            request.header(header::COOKIE, cookie)
        };
        app(state)
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    fn canonical(name: &str) -> CanonicalName {
        CanonicalName::try_from(name).unwrap()
    }

    /// Adds `foobar` 1.0.0, owned by `owner`.
    async fn add_owned_crate(db: &TestDb, owner: &str) {
        TestCrateBuilder::new(db)
            .name("foobar")
            .owner(owner)
            .version("1.0.0")
            .build()
            .await
            .unwrap();
    }

    async fn is_yanked(db: &TestDb, name: &str, version: &str) -> bool {
        db.get_crate_data(&canonical(name))
            .await
            .unwrap()
            .versions
            .iter()
            .find(|v| v.version == version)
            .unwrap()
            .yanked
    }

    async fn register_webhook(db: &TestDb, event: WebhookEvent) {
        db.register_webhook(Webhook {
            id: None,
            event,
            callback_url: "http://127.0.0.1:1/hook".to_string(),
            name: None,
            secret: None,
        })
        .await
        .unwrap();
    }

    async fn webhook_payloads(db: &TestDb) -> Vec<serde_json::Value> {
        db.get_pending_webhook_queue_entries(chrono::Utc::now())
            .await
            .unwrap()
            .into_iter()
            .map(|entry| entry.payload)
            .collect()
    }

    #[tokio::test]
    async fn yank_by_owner_marks_version_yanked_and_notifies_webhooks() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        add_owned_crate(&db, "user").await;
        register_webhook(&db, WebhookEvent::CrateYank).await;

        let response = put_with(state, "/crates/foobar/1.0.0/yank", &cookie).await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(body.is_empty());
        assert!(is_yanked(&db, "foobar", "1.0.0").await);
        let payloads = webhook_payloads(&db).await;
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["type"], "crate_yank");
        assert_eq!(payloads[0]["data"]["crate_version"], "1.0.0");
    }

    #[tokio::test]
    async fn unyank_by_owner_marks_version_not_yanked_and_notifies_webhooks() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        add_owned_crate(&db, "user").await;
        register_webhook(&db, WebhookEvent::CrateUnyank).await;
        db.yank_crate(&canonical("foobar"), &Version::try_from("1.0.0").unwrap())
            .await
            .unwrap();

        let response = put_with(state, "/crates/foobar/1.0.0/unyank", &cookie).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!is_yanked(&db, "foobar", "1.0.0").await);
        let payloads = webhook_payloads(&db).await;
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["type"], "crate_unyank");
    }

    #[tokio::test]
    async fn yank_by_admin_who_is_not_owner() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "other_admin", UserFlags::admin()).await;
        add_owned_crate(&db, "admin").await;

        let response = put_with(state, "/crates/foobar/1.0.0/yank", &cookie).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(is_yanked(&db, "foobar", "1.0.0").await);
    }

    #[tokio::test]
    async fn yank_by_non_owner_is_forbidden() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        add_owned_crate(&db, "admin").await;

        let response = put_with(state, "/crates/foobar/1.0.0/yank", &cookie).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(!is_yanked(&db, "foobar", "1.0.0").await);
    }

    #[tokio::test]
    async fn unyank_by_non_owner_is_forbidden() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        add_owned_crate(&db, "admin").await;
        db.yank_crate(&canonical("foobar"), &Version::try_from("1.0.0").unwrap())
            .await
            .unwrap();

        let response = put_with(state, "/crates/foobar/1.0.0/unyank", &cookie).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(is_yanked(&db, "foobar", "1.0.0").await);
    }

    #[tokio::test]
    async fn yank_by_read_only_owner_is_forbidden() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::read_only()).await;
        add_owned_crate(&db, "user").await;

        let response = put_with(state, "/crates/foobar/1.0.0/yank", &cookie).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(!is_yanked(&db, "foobar", "1.0.0").await);
    }

    #[tokio::test]
    async fn yank_without_session_is_unauthorized() {
        let (state, db) = sqlite_test_state().await;
        add_owned_crate(&db, "admin").await;

        let response = put_with(state, "/crates/foobar/1.0.0/yank", "").await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!is_yanked(&db, "foobar", "1.0.0").await);
    }

    #[tokio::test]
    async fn yank_unknown_version_is_not_found() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;
        add_owned_crate(&db, "user").await;

        let response = put_with(state, "/crates/foobar/2.0.0/yank", &cookie).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn yank_unknown_crate_is_not_found_for_admin() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        let response = put_with(state, "/crates/foobar/1.0.0/yank", &cookie).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // A non-owner gets the same response for a missing crate as for a crate
    // of someone else, so that the existence of a crate is not revealed.
    #[tokio::test]
    async fn yank_unknown_crate_is_forbidden_for_non_owner() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "user", UserFlags::normal()).await;

        let response = put_with(state, "/crates/foobar/1.0.0/yank", &cookie).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn yank_invalid_name_or_version_is_bad_request() {
        let (state, db) = sqlite_test_state().await;
        let cookie = session_cookie(&db, "admin", UserFlags::admin()).await;

        for uri in ["/crates/foo$bar/1.0.0/yank", "/crates/foobar/one/yank"] {
            let response = put_with(state.clone(), uri, &cookie).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
        }
    }
}
