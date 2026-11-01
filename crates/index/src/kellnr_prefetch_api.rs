use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use kellnr_common::original_name::OriginalName;
use kellnr_common::prefetch::{Prefetch, db_date_to_http_date, is_not_modified, quote_etag};
use kellnr_db::DbProvider;
use kellnr_db::error::DbError;
use kellnr_http::authz::can_read_crate;
use kellnr_http::error::ApiResult;
use kellnr_http::identity::{AnyUser, Identity};
use kellnr_http::state::{DbState, SettingsState};
use tracing::error;

use super::config_json::{ConfigJson, IndexKind};
use crate::prefetch_response::PrefetchResponse;

/// Get registry configuration
///
/// Returns the configuration JSON for the Kellnr registry index.
#[utoipa::path(
    get,
    path = "/config.json",
    tag = "crates",
    responses(
        (status = 200, description = "Registry configuration")
    ),
    security(("cargo_token" = []))
)]
#[allow(clippy::unused_async)] // part of the router
pub async fn config_kellnr(State(settings): SettingsState) -> Json<ConfigJson> {
    Json(ConfigJson::new(&settings, IndexKind::Kellnr))
}

/// Prefetch crate index data
///
/// Returns sparse index data for a crate (3+ character names).
#[utoipa::path(
    get,
    path = "/{a}/{b}/{package}",
    tag = "crates",
    params(
        ("a" = String, Path, description = "First path segment"),
        ("b" = String, Path, description = "Second path segment"),
        ("package" = String, Path, description = "Package name")
    ),
    responses(
        (status = 200, description = "Crate index data"),
        (status = 304, description = "Not modified"),
        (status = 404, description = "Crate not found or download-restricted without access")
    ),
    security(("cargo_token" = []))
)]
pub async fn prefetch_kellnr(
    Path((_a, _b, package)): Path<(String, String, OriginalName)>,
    headers: HeaderMap,
    State(db): DbState,
    user: Option<AnyUser>,
) -> ApiResult<PrefetchResponse> {
    PrefetchResponse::from_result(
        internal_kellnr_prefetch(&package, &headers, &db, user.as_deref()).await,
    )
}

/// Prefetch crate index data for short names
///
/// Returns sparse index data for a crate (1-2 character names).
#[utoipa::path(
    get,
    path = "/{a}/{package}",
    tag = "crates",
    params(
        ("a" = String, Path, description = "First path segment"),
        ("package" = String, Path, description = "Package name")
    ),
    responses(
        (status = 200, description = "Crate index data"),
        (status = 304, description = "Not modified"),
        (status = 404, description = "Crate not found or download-restricted without access")
    ),
    security(("cargo_token" = []))
)]
pub async fn prefetch_len2_kellnr(
    Path((_a, package)): Path<(String, OriginalName)>,
    headers: HeaderMap,
    State(db): DbState,
    user: Option<AnyUser>,
) -> ApiResult<PrefetchResponse> {
    PrefetchResponse::from_result(
        internal_kellnr_prefetch(&package, &headers, &db, user.as_deref()).await,
    )
}

/// Index entries of download-restricted crates are only served to callers with
/// access. Cargo sends credentials on index requests only if `config.json`
/// reports `auth-required`, which is also required to download such crates.
async fn internal_kellnr_prefetch(
    name: &OriginalName,
    headers: &HeaderMap,
    db: &Arc<dyn DbProvider>,
    user: Option<&Identity>,
) -> Result<Prefetch, StatusCode> {
    // Same response as a missing crate to not reveal that a restricted crate exists.
    match can_read_crate(&name.to_canonical(), user, db).await {
        Ok(true) => {}
        Ok(false) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to check access to crate {name}: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
    let prefetch = match db.get_prefetch_data(name).await {
        Ok(prefetch) => prefetch,
        Err(DbError::CrateNotFound(_)) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to get the index data of crate {name}: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let value = |name: header::HeaderName| headers.get(name).and_then(|h| h.to_str().ok());
    if is_not_modified(
        value(header::IF_NONE_MATCH),
        value(header::IF_MODIFIED_SINCE),
        &prefetch.etag,
        &prefetch.last_modified,
    ) {
        return Err(StatusCode::NOT_MODIFIED);
    }

    Ok(Prefetch {
        etag: quote_etag(&prefetch.etag),
        last_modified: db_date_to_http_date(&prefetch.last_modified),
        data: prefetch.data,
    })
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, header};
    use axum::routing::get;
    use http_body_util::BodyExt;
    use kellnr_common::account_name::UserName;
    use kellnr_common::plain_token::PlainToken;
    use kellnr_db::UserFlags;
    use kellnr_db::test_utils::{TestCrateBuilder, test_drop_table};
    use kellnr_http::state::AppStateData;
    use kellnr_http::test_support::sqlite_test_state;
    use kellnr_settings::Protocol;
    use kellnr_test_db::TestDb;
    use tower::ServiceExt;

    use super::*;

    /// `Last-Modified` of the crates added by `TestCrateBuilder`.
    const LAST_MODIFIED: &str = "Wed, 07 Oct 2020 13:18:00 GMT";

    #[tokio::test]
    async fn config_returns_config_json() {
        let (app, _db) = app().await;
        let r = app
            .oneshot(
                Request::get("/api/v1/index/config.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            r#"{"dl":"http://test.api.com:1234/api/v1/crates/dl","api":"http://test.api.com:1234","auth-required":false}"#,
            std::str::from_utf8(&result_msg).unwrap()
        );
    }

    async fn stored_prefetch(db: &TestDb, name: &str) -> Prefetch {
        db.get_prefetch_data(&OriginalName::try_from(name).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn prefetch_returns_prefetch_data() {
        let (app, db) = app().await;
        let stored = stored_prefetch(&db, "metadata").await;
        let r = app
            .oneshot(
                Request::get("/api/v1/index/me/ta/metadata")
                    .header(header::IF_MODIFIED_SINCE, "foo")
                    .header(header::IF_NONE_MATCH, "bar")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_status = r.status();

        assert_eq!(StatusCode::OK, result_status);
        assert_eq!(
            stored.data.len().to_string(),
            r.headers()
                .get(header::CONTENT_LENGTH)
                .unwrap()
                .to_str()
                .unwrap()
        );
        assert_eq!(
            LAST_MODIFIED,
            r.headers().get(header::LAST_MODIFIED).unwrap()
        );
        assert_eq!(
            quote_etag(&stored.etag),
            r.headers().get(header::ETAG).unwrap().to_str().unwrap()
        );
        assert_eq!(
            stored.data,
            r.into_body().collect().await.unwrap().to_bytes()
        );
    }

    /// Status of a request for "metadata" with the headers. `ETAG` in a
    /// header value is replaced with the stored entity tag.
    async fn prefetch_status(headers: &[(header::HeaderName, &str)]) -> StatusCode {
        let (app, db) = app().await;
        let etag = quote_etag(&stored_prefetch(&db, "metadata").await.etag);
        let mut request = Request::get("/api/v1/index/me/ta/metadata");
        for (name, value) in headers {
            request = request.header(name, value.replace("ETAG", &etag));
        }
        app.oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn prefetch_returns_not_modified() {
        let status = prefetch_status(&[
            (header::IF_MODIFIED_SINCE, LAST_MODIFIED),
            (header::IF_NONE_MATCH, "ETAG"),
        ])
        .await;

        assert_eq!(StatusCode::NOT_MODIFIED, status);
    }

    #[tokio::test]
    async fn prefetch_returns_not_modified_for_matching_etag_alone() {
        let status = prefetch_status(&[(header::IF_NONE_MATCH, "ETAG")]).await;

        assert_eq!(StatusCode::NOT_MODIFIED, status);
    }

    #[tokio::test]
    async fn prefetch_returns_not_modified_for_matching_date_alone() {
        let status = prefetch_status(&[(header::IF_MODIFIED_SINCE, LAST_MODIFIED)]).await;

        assert_eq!(StatusCode::NOT_MODIFIED, status);
    }

    #[tokio::test]
    async fn prefetch_returns_data_for_older_date() {
        let status =
            prefetch_status(&[(header::IF_MODIFIED_SINCE, "Tue, 06 Oct 2020 13:18:00 GMT")]).await;

        assert_eq!(StatusCode::OK, status);
    }

    #[tokio::test]
    async fn prefetch_returns_data_for_other_etag_with_matching_date() {
        let status = prefetch_status(&[
            (header::IF_MODIFIED_SINCE, LAST_MODIFIED),
            (header::IF_NONE_MATCH, "\"old\""),
        ])
        .await;

        assert_eq!(StatusCode::OK, status);
    }

    #[tokio::test]
    async fn prefetch_returns_internal_error_on_database_failure() {
        let (app, db) = app().await;
        test_drop_table(&db, "krate").await.unwrap();

        let r = app
            .oneshot(
                Request::get("/api/v1/index/me/ta/metadata")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, r.status());
    }

    #[tokio::test]
    async fn prefetch_restricted_crate_anonymous_returns_not_found() {
        let (app, _db) = app().await;
        let r = app
            .oneshot(
                Request::get("/api/v1/index/se/cr/secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, r.status());
    }

    #[tokio::test]
    async fn prefetch_restricted_crate_with_access_returns_prefetch_data() {
        let (app, _db) = app().await;
        let r = app
            .oneshot(
                Request::get("/api/v1/index/se/cr/secret")
                    .header(header::AUTHORIZATION, "crate-user-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::OK, r.status());
    }

    #[tokio::test]
    async fn prefetch_returns_not_found() {
        let (app, _db) = app().await;
        let r = app
            .oneshot(
                Request::get("/api/v1/index/no/tf/notfound")
                    .header(header::IF_MODIFIED_SINCE, "date")
                    .header(header::IF_NONE_MATCH, "etag")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(StatusCode::NOT_FOUND, r.status());
    }

    /// The index routes on a database with the crate "metadata" and the
    /// download-restricted crate "secret", which only the token
    /// `crate-user-token` of "`crate_user`" may read.
    async fn app() -> (Router, TestDb) {
        let mut settings = kellnr_settings::test_settings();
        settings.origin = kellnr_settings::Origin {
            protocol: Protocol::Http,
            hostname: "test.api.com".to_string(),
            port: 1234,
            path: String::new(),
            cookie_secure: None,
        };

        let (state, db) = sqlite_test_state().await;
        for name in ["metadata", "secret"] {
            TestCrateBuilder::new(&db).name(name).build().await.unwrap();
        }
        let secret = OriginalName::try_from("secret").unwrap().to_canonical();
        let crate_user = UserName::new("crate_user").unwrap();
        db.change_download_restricted(&secret, true).await.unwrap();
        db.add_user(&crate_user, "pwd", UserFlags::normal())
            .await
            .unwrap();
        db.add_auth_token("test", &PlainToken::from("crate-user-token"), &crate_user)
            .await
            .unwrap();
        db.add_crate_user(&secret, &crate_user).await.unwrap();

        let kellnr_prefetch = Router::new()
            .route("/config.json", get(config_kellnr))
            .route("/{a}/{b}/{name}", get(prefetch_kellnr))
            .route("/{a}/{name}", get(prefetch_len2_kellnr));

        let state = AppStateData {
            settings: Arc::new(settings),
            ..state
        };

        let app = Router::new()
            .nest("/api/v1/index", kellnr_prefetch)
            .with_state(state);
        (app, db)
    }
}
