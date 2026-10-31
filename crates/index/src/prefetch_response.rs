use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use kellnr_common::prefetch::Prefetch;
use kellnr_http::error::ApiResult;

/// The answer to a sparse index request.
pub enum PrefetchResponse {
    /// The index entry with its caching headers.
    Entry(Prefetch),
    /// The client already has the current entry.
    NotModified,
}

impl PrefetchResponse {
    /// A 304 for a conditional request is no error, every other status is.
    pub fn from_result(result: Result<Prefetch, StatusCode>) -> ApiResult<Self> {
        match result {
            Ok(prefetch) => Ok(Self::Entry(prefetch)),
            Err(StatusCode::NOT_MODIFIED) => Ok(Self::NotModified),
            Err(status) => Err(status.into()),
        }
    }
}

impl IntoResponse for PrefetchResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Entry(prefetch) => Response::builder()
                .header("Content-Length", prefetch.data.len().to_string())
                .header("ETag", prefetch.etag)
                .header("Last-Modified", prefetch.last_modified)
                .status(StatusCode::OK)
                .body(Body::from(prefetch.data))
                .unwrap()
                .into_response(),
            Self::NotModified => StatusCode::NOT_MODIFIED.into_response(),
        }
    }
}
