use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use kellnr_common::prefetch::Prefetch;

/// A sparse index entry with its caching headers.
pub struct PrefetchResponse(pub Prefetch);

impl IntoResponse for PrefetchResponse {
    fn into_response(self) -> Response {
        let prefetch = self.0;
        Response::builder()
            .header("Content-Length", prefetch.data.len().to_string())
            .header("ETag", prefetch.etag)
            .header("Last-Modified", prefetch.last_modified)
            .status(StatusCode::OK)
            .body(Body::from(prefetch.data))
            .unwrap()
            .into_response()
    }
}
