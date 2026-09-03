use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Router, middleware};
use kellnr_appstate::AppStateData;
use kellnr_auth::oauth2::OAuth2Handler;
use kellnr_embedded_resources::{embedded_static_handler, embedded_static_root_handler};
use kellnr_settings::Registry;
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

pub fn create_router(
    state: AppStateData,
    max_docs_size: usize,
    max_crate_size: usize,
    max_toolchain_size: usize,
    oauth2_handler: Option<Arc<OAuth2Handler>>,
) -> Router {
    // Docs are served from the pluggable Storage backend, not embedded assets.
    let docs_service: Router<AppStateData> = Router::new()
        .route("/{*path}", get(docs_static::serve_doc_file))
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
                kellnr_api_routes::create_routes(state.clone(), max_crate_size, download_semaphore),
            )
            .nest("/api/v1", health_routes::create_routes());

    // Conditionally add toolchain routes if enabled
    if state.settings.toolchain.enabled {
        api_router = api_router
            .nest(
                "/api/v1/toolchains",
                toolchain_routes::create_api_routes(state.clone(), max_toolchain_size),
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
            docs_routes::create_manual_routes(max_docs_size),
        )
        .nest("/docs", docs_service)
        // Serve Swagger UI at /api/docs with OpenAPI spec at /api/openapi.json
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", api));

    router
        // Always serve the UI from the embedded directory (single-binary deploy).
        .fallback(get(embedded_static_handler))
        .with_state(state)
        // Add OAuth2 handler as an extension (accessible via Extension<Option<Arc<OAuth2Handler>>>)
        .layer(Extension(oauth2_handler))
        // Baseline security headers on every response.
        .layer(middleware::map_response(add_security_headers))
        .layer(tower_http::trace::TraceLayer::new_for_http())
}

/// Add baseline security response headers to every response.
///
/// `nosniff` stops MIME sniffing, `SAMEORIGIN` blocks cross-origin framing
/// (clickjacking) while still allowing the UI to embed same-origin content,
/// and `no-referrer` avoids leaking internal URLs to third parties.
async fn add_security_headers(mut response: axum::response::Response) -> axum::response::Response {
    use axum::http::HeaderValue;
    use axum::http::header::{REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS};

    let headers = response.headers_mut();
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("SAMEORIGIN"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    response
}

/// Content-Security-Policy for the user-uploaded documentation served under
/// `/docs`. `connect-src 'none'` and `form-action 'none'` neutralize the
/// cross-endpoint attack path (a malicious docs page calling the authenticated
/// API), while `'unsafe-inline'` scripts/styles are still allowed so generated
/// rustdoc keeps working. True isolation would need a separate origin.
async fn add_docs_csp(mut response: axum::response::Response) -> axum::response::Response {
    use axum::http::HeaderValue;
    use axum::http::header::CONTENT_SECURITY_POLICY;

    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; \
             style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; \
             connect-src 'none'; form-action 'none'; frame-ancestors 'self'; \
             base-uri 'none'; object-src 'none'",
        ),
    );
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
    use bytes::Bytes;
    use kellnr_storage::docs_storage::DocsStorage;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn docs_route_serves_stored_file_with_mime_and_csp() {
        let state = kellnr_appstate::test_state();
        let key = DocsStorage::file_key(
            "routes-test-crate",
            "1.0.0",
            "doc/routes_test_crate/index.html",
        );
        state
            .docs_storage
            .put(&key, Bytes::from_static(b"<html>hi</html>"))
            .await
            .unwrap();

        let app = create_router(state, 100, 100, 100, None);

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
        assert!(r.headers().contains_key(header::CONTENT_SECURITY_POLICY));
        assert!(r.headers().contains_key(header::ETAG));
        assert!(r.headers().contains_key(header::LAST_MODIFIED));
    }

    #[tokio::test]
    async fn docs_route_returns_304_for_matching_if_none_match() {
        let state = kellnr_appstate::test_state();
        let key = DocsStorage::file_key(
            "routes-test-crate",
            "1.0.0",
            "doc/routes_test_crate/index.html",
        );
        state
            .docs_storage
            .put(&key, Bytes::from_static(b"<html>hi</html>"))
            .await
            .unwrap();

        let app = create_router(state, 100, 100, 100, None);

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
        let state = kellnr_appstate::test_state();
        let app = create_router(state, 100, 100, 100, None);

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
