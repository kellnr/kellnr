use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Router, middleware};
use kellnr_appstate::AppStateData;
use kellnr_auth::oauth2::OAuth2Handler;
use kellnr_embedded_resources::{embedded_static_handler, embedded_static_root_handler};
use kellnr_settings::{Protocol, Registry};
use kellnr_web_ui::session;
use tokio::sync::Semaphore;
use tower_http::timeout::TimeoutLayer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

use crate::openapi::ApiDoc;

mod auth_routes;
mod crate_access_routes;
mod cratesio_api_routes;
mod docs_routes;
mod docs_static;
mod group_routes;
mod health_routes;
mod kellnr_api_routes;
mod oauth2_routes;
mod toolchain_routes;
mod ui_routes;
mod user_routes;
mod webhook_routes;

pub fn create_router(state: AppStateData, oauth2_handler: Option<Arc<OAuth2Handler>>) -> Router {
    let max_docs_bytes = state.settings.docs.max_bytes();
    let max_crate_bytes = state.settings.registry.max_crate_bytes();
    let max_toolchain_bytes = state.settings.toolchain.max_bytes();

    // Docs are served from the pluggable Storage backend, not embedded assets.
    let docs_service: Router<AppStateData> = Router::new()
        .route("/{*path}", get(docs_static::serve_doc_file))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            docs_static::restrict_docs_access,
        ))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            session::session_auth_when_required,
        ))
        // User-uploaded docs are served from the same origin as the UI and API.
        // A restrictive CSP keeps rustdoc rendering while stopping any script in
        // the uploaded content from reaching the API or exfiltrating data.
        .layer(middleware::map_response(add_docs_csp));

    // Shared download concurrency limiter across kellnr and crates.io routes
    let download_semaphore = if state.settings.registry.download_max_concurrent > 0 {
        Some(Arc::new(Semaphore::new(
            state.settings.registry.download_max_concurrent,
        )))
    } else {
        None
    };

    // Build API routes using OpenApiRouter with the base OpenAPI document
    let mut api_router: OpenApiRouter<AppStateData> =
        OpenApiRouter::with_openapi(ApiDoc::openapi())
            .route("/", get(embedded_static_root_handler))
            .nest("/api/v1/ui", ui_routes::create_routes(state.clone()))
            .nest("/api/v1/auth", auth_routes::create_routes())
            .nest("/api/v1/users", user_routes::create_routes())
            .nest("/api/v1/groups", group_routes::create_routes())
            .nest("/api/v1/acl", crate_access_routes::create_routes())
            .nest("/api/v1/docs", docs_routes::create_ui_routes(state.clone()))
            .nest("/api/v1/webhooks", webhook_routes::create_routes())
            .nest("/api/v1/oauth2", oauth2_routes::create_routes())
            .nest(
                "/api/v1/cratesio",
                cratesio_api_routes::create_routes(state.clone(), download_semaphore.clone()),
            )
            .nest(
                "/api/v1/crates",
                kellnr_api_routes::create_routes(
                    state.clone(),
                    max_crate_bytes,
                    download_semaphore,
                ),
            )
            .nest("/api/v1", health_routes::create_routes());

    // Conditionally add toolchain routes if enabled
    if state.settings.toolchain.enabled {
        api_router = api_router
            .nest(
                "/api/v1/toolchains",
                toolchain_routes::create_api_routes(state.clone(), max_toolchain_bytes),
            )
            .nest(
                "/api/v1/toolchains/dist",
                toolchain_routes::create_dist_routes(state.clone()),
            );
    }

    // Split into router and OpenAPI spec
    let (router, api): (Router<AppStateData>, _) = api_router.split_for_parts();

    // Add routes that don't have OpenAPI annotations and static services
    let router = router
        .nest(
            "/api/v1/docs",
            docs_routes::create_manual_routes(max_docs_bytes),
        )
        .nest("/docs", docs_service)
        // Serve Swagger UI at /api/docs with OpenAPI spec at /api/openapi.json
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", api));

    let use_hsts = state.settings.origin.protocol == Protocol::Https;
    router
        // Always serve the UI from the embedded directory (single-binary deploy).
        .fallback(get(embedded_static_handler))
        .with_state(state)
        // Add OAuth2 handler as an extension (accessible via Extension<Option<Arc<OAuth2Handler>>>)
        .layer(Extension(oauth2_handler))
        // Baseline security headers on every response.
        .layer(middleware::map_response_with_state(
            use_hsts,
            add_security_headers,
        ))
        .layer(tower_http::trace::TraceLayer::new_for_http())
}

/// Content-Security-Policy for the UI, the Swagger UI and the API. Vuetify and
/// Swagger UI set inline styles, crate READMEs may embed images from any host,
/// and everything else is served from the same origin.
const UI_CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
     img-src 'self' data: http: https:; font-src 'self' data:; connect-src 'self'; \
     frame-ancestors 'self'; form-action 'self'; object-src 'none'; base-uri 'self'";

/// Add baseline security response headers to every response.
///
/// `nosniff` stops MIME sniffing, `SAMEORIGIN` blocks cross-origin framing
/// (clickjacking) while still allowing the UI to embed same-origin content,
/// and `no-referrer` avoids leaking internal URLs to third parties.
/// Responses without their own Content-Security-Policy (all but `/docs`) get
/// [`UI_CSP`] and `Cross-Origin-Opener-Policy`, which keeps windows opened by
/// or opening a docs page from getting a handle to the UI (secure contexts
/// only). HSTS is only sent if Kellnr is configured to be reached via HTTPS.
async fn add_security_headers(
    State(use_hsts): State<bool>,
    mut response: axum::response::Response,
) -> axum::response::Response {
    use axum::http::HeaderValue;
    use axum::http::header::{
        CONTENT_SECURITY_POLICY, REFERRER_POLICY, STRICT_TRANSPORT_SECURITY,
        X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
    };

    let headers = response.headers_mut();
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("SAMEORIGIN"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    if !headers.contains_key(CONTENT_SECURITY_POLICY) {
        headers.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(UI_CSP));
        headers.insert(
            "cross-origin-opener-policy",
            HeaderValue::from_static("same-origin"),
        );
    }
    if use_hsts {
        headers.insert(
            STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000"),
        );
    }
    response
}

/// Content-Security-Policy for the user-uploaded documentation served under
/// `/docs`. `'unsafe-inline'` scripts/styles are allowed so generated rustdoc
/// keeps working, but scripts must not reach the authenticated UI or API:
/// - `connect-src 'none'` and `form-action 'none'` block requests to the API.
/// - `frame-src`, `child-src` and `worker-src 'none'` block same-origin frames
///   and workers.
/// - `sandbox` without `allow-popups` blocks `window.open` and `_blank` links,
///   so a docs page cannot get a handle to a same-origin UI window.
///
/// `allow-same-origin` is required: an opaque origin would make browsers drop
/// the `SameSite=Strict` session cookie on all docs subresources, which breaks
/// docs with `auth_required` or a download restriction. True isolation would
/// need a separate origin for docs.
const DOCS_CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline'; \
     style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; \
     connect-src 'none'; form-action 'none'; frame-src 'none'; child-src 'none'; \
     worker-src 'none'; frame-ancestors 'self'; base-uri 'none'; object-src 'none'; \
     sandbox allow-scripts allow-same-origin";

async fn add_docs_csp(mut response: axum::response::Response) -> axum::response::Response {
    use axum::http::HeaderValue;
    use axum::http::header::CONTENT_SECURITY_POLICY;

    response
        .headers_mut()
        .insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(DOCS_CSP));
    response
}

/// Apply download concurrency and timeout limits to a router.
///
/// When the semaphore is provided, requests that exceed the concurrency limit
/// wait for a permit before they enter the handler.
/// When `download_timeout_seconds > 0`, requests that exceed the timeout
/// receive 504 Gateway Timeout.
///
/// Layer ordering: the timeout wraps the handler directly, so it measures
/// actual request processing time (not time spent waiting for a permit).
pub(crate) fn apply_download_limits(
    mut router: Router<AppStateData>,
    semaphore: Option<Arc<Semaphore>>,
    settings: &Registry,
) -> Router<AppStateData> {
    // Apply timeout first (innermost layer = applied first to the handler)
    if settings.download_timeout_seconds > 0 {
        let timeout_secs = settings.download_timeout_seconds;
        router = router
            .layer(middleware::map_response(
                move |response: axum::response::Response| async move {
                    if response.status() == StatusCode::GATEWAY_TIMEOUT {
                        tracing::warn!(
                            "Download request timed out after {timeout_secs}s. \
                             Consider increasing registry.download_timeout_seconds"
                        );
                    }
                    response
                },
            ))
            .layer(TimeoutLayer::with_status_code(
                StatusCode::GATEWAY_TIMEOUT,
                Duration::from_secs(timeout_secs),
            ));
    }

    // Apply semaphore second (outermost layer = checked before timeout starts)
    if let Some(semaphore) = semaphore {
        router = router.layer(middleware::from_fn(
            move |req: axum::extract::Request, next: Next| {
                let sem = semaphore.clone();
                async move {
                    match sem.acquire_owned().await {
                        Ok(_permit) => next.run(req).await,
                        Err(error) => {
                            tracing::error!("Download semaphore closed unexpectedly: {error}");
                            StatusCode::SERVICE_UNAVAILABLE.into_response()
                        }
                    }
                }
            },
        ));
    }

    router
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum_extra::extract::cookie::Key;
    use bytes::Bytes;
    use kellnr_appstate::{TEST_KEY, encode_cookies};
    use kellnr_common::canonical_name::CanonicalName;
    use kellnr_common::version::Version;
    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;
    use kellnr_db::{SessionInfo, User};
    use kellnr_settings::constants::COOKIE_SESSION_ID;
    use kellnr_storage::docs_storage::{DocsName, DocsStorage};
    use mockall::predicate::eq;
    use tower::ServiceExt;

    use super::*;

    fn docs_test_state(db: MockDb) -> AppStateData {
        AppStateData {
            db: Arc::new(db),
            signing_key: Key::from(TEST_KEY),
            ..kellnr_appstate::test_state()
        }
    }

    fn unrestricted_db() -> MockDb {
        let mut db = MockDb::new();
        db.expect_is_download_restricted().returning(|_| Ok(false));
        db
    }

    /// Db for a restricted crate `secret` with crate user `alice`, owner
    /// `olivia` and group member `greg`.
    fn restricted_db() -> MockDb {
        let mut db = MockDb::new();
        db.expect_is_download_restricted()
            .with(eq(CanonicalName::try_from("secret").unwrap()))
            .returning(|_| Ok(true));
        db.expect_get_user_from_token().returning(|token| {
            Ok(User {
                name: token.to_string(),
                ..User::default()
            })
        });
        db.expect_is_crate_user()
            .returning(|_, user| Ok(user == "alice"));
        db.expect_is_crate_group_user()
            .returning(|_, user| Ok(user == "greg"));
        db.expect_is_owner()
            .returning(|_, user| Ok(user == "olivia"));
        db
    }

    async fn put_secret_docs(state: &AppStateData) -> String {
        let key = DocsStorage::file_key(
            &DocsName::from_folder("secret".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            "src/secret/lib.rs.html",
        );
        state
            .docs_storage
            .put(&key, Bytes::from_static(b"<html>source</html>"))
            .await
            .unwrap();
        format!("/docs/{key}")
    }

    async fn get_docs(state: AppStateData, request: Request<Body>) -> StatusCode {
        create_router(state, None)
            .oneshot(request)
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn restricted_docs_hidden_from_anonymous_user() {
        let state = docs_test_state(restricted_db());
        let uri = put_secret_docs(&state).await;

        let status = get_docs(state, Request::get(uri).body(Body::empty()).unwrap()).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn restricted_docs_hidden_from_user_without_access() {
        let state = docs_test_state(restricted_db());
        let uri = put_secret_docs(&state).await;

        let request = Request::get(uri)
            .header(header::AUTHORIZATION, "bob")
            .body(Body::empty())
            .unwrap();
        let status = get_docs(state, request).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    async fn get_secret_docs_as(user: &str) -> StatusCode {
        let state = docs_test_state(restricted_db());
        let uri = put_secret_docs(&state).await;

        let request = Request::get(uri)
            .header(header::AUTHORIZATION, user)
            .body(Body::empty())
            .unwrap();
        get_docs(state, request).await
    }

    #[tokio::test]
    async fn restricted_docs_served_to_crate_user() {
        assert_eq!(get_secret_docs_as("alice").await, StatusCode::OK);
    }

    #[tokio::test]
    async fn restricted_docs_served_to_owner() {
        assert_eq!(get_secret_docs_as("olivia").await, StatusCode::OK);
    }

    #[tokio::test]
    async fn restricted_docs_served_to_crate_group_member() {
        assert_eq!(get_secret_docs_as("greg").await, StatusCode::OK);
    }

    #[tokio::test]
    async fn docs_hidden_when_restriction_check_fails() {
        let mut db = MockDb::new();
        db.expect_is_download_restricted()
            .returning(|_| Err(DbError::CrateNotFound("secret".to_string())));
        let state = docs_test_state(db);
        let uri = put_secret_docs(&state).await;

        let status = get_docs(state, Request::get(uri).body(Body::empty()).unwrap()).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn restricted_docs_hidden_when_access_check_fails() {
        let mut db = MockDb::new();
        db.expect_is_download_restricted().returning(|_| Ok(true));
        db.expect_get_user_from_token()
            .returning(|_| Ok(User::default()));
        db.expect_is_crate_user()
            .returning(|_, _| Err(DbError::CrateNotFound("secret".to_string())));
        let state = docs_test_state(db);
        let uri = put_secret_docs(&state).await;

        let request = Request::get(uri)
            .header(header::AUTHORIZATION, "alice")
            .body(Body::empty())
            .unwrap();
        let status = get_docs(state, request).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn restricted_docs_served_to_admin_session() {
        let mut db = MockDb::new();
        db.expect_is_download_restricted().returning(|_| Ok(true));
        db.expect_validate_session()
            .with(eq("admin-session"))
            .returning(|_| {
                Ok(SessionInfo {
                    name: "admin".to_string(),
                    is_admin: true,
                    is_read_only: false,
                })
            });
        let state = docs_test_state(db);
        let uri = put_secret_docs(&state).await;

        let request = Request::get(uri)
            .header(
                header::COOKIE,
                encode_cookies([(COOKIE_SESSION_ID, "admin-session")]),
            )
            .body(Body::empty())
            .unwrap();
        let status = get_docs(state, request).await;

        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn restricted_docs_checked_with_canonical_crate_name() {
        let state = docs_test_state(restricted_db());
        let key = DocsStorage::file_key(
            &DocsName::from_folder("Secret".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            "src/secret/lib.rs.html",
        );
        state
            .docs_storage
            .put(&key, Bytes::from_static(b"<html>source</html>"))
            .await
            .unwrap();

        let status = get_docs(
            state,
            Request::get(format!("/docs/{key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn docs_path_with_empty_segment_is_rejected() {
        // The storage drops empty segments, so this path addresses `secret/...`.
        // It must never be served based on a check of a different segment.
        let state = docs_test_state(MockDb::new());
        let uri = put_secret_docs(&state).await.replace("/docs/", "/docs//");

        let status = get_docs(state, Request::get(uri).body(Body::empty()).unwrap()).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn restricted_docs_never_answer_conditional_request_without_access() {
        let state = docs_test_state(restricted_db());
        let uri = put_secret_docs(&state).await;
        let app = create_router(state, None);

        let allowed = app
            .clone()
            .oneshot(
                Request::get(&uri)
                    .header(header::AUTHORIZATION, "alice")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let etag = allowed.headers().get(header::ETAG).unwrap().clone();

        let denied = app
            .oneshot(
                Request::get(&uri)
                    .header(header::AUTHORIZATION, "bob")
                    .header(header::IF_NONE_MATCH, etag)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(denied.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn docs_route_serves_stored_file_with_mime_and_csp() {
        let state = docs_test_state(unrestricted_db());
        let key = DocsStorage::file_key(
            &DocsName::from_folder("routes-test-crate".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            "doc/routes_test_crate/index.html",
        );
        state
            .docs_storage
            .put(&key, Bytes::from_static(b"<html>hi</html>"))
            .await
            .unwrap();

        let app = create_router(state, None);

        let r = app
            .oneshot(
                Request::get(format!("/docs/{key}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers().get(header::CONTENT_TYPE).unwrap(), "text/html");
        assert_eq!(r.headers()[header::CONTENT_SECURITY_POLICY], DOCS_CSP);
        assert!(r.headers().contains_key(header::ETAG));
        assert!(r.headers().contains_key(header::LAST_MODIFIED));
    }

    #[test]
    fn docs_csp_blocks_popups_frames_and_workers() {
        let directives: Vec<&str> = DOCS_CSP.split(';').map(str::trim).collect();

        assert!(directives.contains(&"sandbox allow-scripts allow-same-origin"));
        assert!(!DOCS_CSP.contains("allow-popups"));
        for directive in [
            "frame-src 'none'",
            "child-src 'none'",
            "worker-src 'none'",
            "connect-src 'none'",
        ] {
            assert!(directives.contains(&directive), "missing {directive}");
        }
    }

    async fn get_response(state: AppStateData, uri: &str) -> axum::response::Response {
        create_router(state, None)
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn ui_responses_have_security_headers() {
        let r = get_response(kellnr_appstate::test_state(), "/").await;

        assert_eq!(r.headers()[header::CONTENT_SECURITY_POLICY], UI_CSP);
        assert_eq!(r.headers()["cross-origin-opener-policy"], "same-origin");
        assert_eq!(r.headers()[header::X_FRAME_OPTIONS], "SAMEORIGIN");
        assert_eq!(r.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert!(!r.headers().contains_key(header::STRICT_TRANSPORT_SECURITY));
    }

    #[tokio::test]
    async fn api_responses_have_ui_csp() {
        let r = get_response(kellnr_appstate::test_state(), "/api/v1/health").await;

        assert_eq!(r.headers()[header::CONTENT_SECURITY_POLICY], UI_CSP);
    }

    #[tokio::test]
    async fn swagger_ui_has_ui_csp() {
        let r = get_response(kellnr_appstate::test_state(), "/api/docs/").await;

        assert_eq!(r.headers()[header::CONTENT_SECURITY_POLICY], UI_CSP);
    }

    #[test]
    fn ui_csp_restricts_scripts_objects_and_framing() {
        let directives: Vec<&str> = UI_CSP.split(';').map(str::trim).collect();

        for directive in [
            "script-src 'self'",
            "object-src 'none'",
            "base-uri 'self'",
            "frame-ancestors 'self'",
        ] {
            assert!(directives.contains(&directive), "missing {directive}");
        }
    }

    #[tokio::test]
    async fn hsts_is_sent_when_origin_uses_https() {
        let mut state = kellnr_appstate::test_state();
        let mut settings = (*state.settings).clone();
        settings.origin.protocol = Protocol::Https;
        state.settings = Arc::new(settings);

        let r = get_response(state, "/").await;

        assert_eq!(
            r.headers()[header::STRICT_TRANSPORT_SECURITY],
            "max-age=31536000"
        );
    }

    #[tokio::test]
    async fn docs_keep_their_own_csp() {
        let state = docs_test_state(unrestricted_db());
        let uri = put_secret_docs(&state).await;

        let r = get_response(state, &uri).await;

        assert_eq!(r.headers()[header::CONTENT_SECURITY_POLICY], DOCS_CSP);
        assert!(!r.headers().contains_key("cross-origin-opener-policy"));
    }

    async fn cache_control_for(file: &str) -> String {
        let state = docs_test_state(unrestricted_db());
        let key = DocsStorage::file_key(
            &DocsName::from_folder("routes-test-crate".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            file,
        );
        state
            .docs_storage
            .put(&key, Bytes::from_static(b"content"))
            .await
            .unwrap();

        let r = create_router(state, None)
            .oneshot(
                Request::get(format!("/docs/{key}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
        r.headers()[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    async fn docs_pages_are_always_revalidated() {
        assert_eq!(
            cache_control_for("doc/routes_test_crate/index.html").await,
            "private, no-cache"
        );
    }

    #[tokio::test]
    async fn rustdoc_static_files_are_cached_by_browser() {
        assert_eq!(
            cache_control_for("doc/static.files/rustdoc-1a2b3c4d.css").await,
            "private, max-age=31536000, immutable"
        );
    }

    #[tokio::test]
    async fn docs_route_returns_304_for_matching_if_none_match() {
        let state = docs_test_state(unrestricted_db());
        let key = DocsStorage::file_key(
            &DocsName::from_folder("routes-test-crate".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            "doc/routes_test_crate/index.html",
        );
        state
            .docs_storage
            .put(&key, Bytes::from_static(b"<html>hi</html>"))
            .await
            .unwrap();

        let app = create_router(state, None);

        let first = app
            .clone()
            .oneshot(
                Request::get(format!("/docs/{key}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let etag = first.headers().get(header::ETAG).unwrap().clone();

        let second = app
            .oneshot(
                Request::get(format!("/docs/{key}"))
                    .header(header::IF_NONE_MATCH, etag)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
        let body = axum::body::to_bytes(second.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn docs_route_404_for_missing_file() {
        let state = docs_test_state(unrestricted_db());
        let app = create_router(state, None);

        let r = app
            .oneshot(
                Request::get("/docs/routes-test-crate/does-not-exist/index.html")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }
}
