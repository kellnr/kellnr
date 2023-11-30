use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

#[derive(Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct Prefetch {
    pub data: Vec<u8>,
    pub etag: String,
    pub last_modified: String,
}

impl IntoResponse for Prefetch {
    fn into_response(self) -> Response {
        Response::builder()
            .header("Content-Length", self.data.len().to_string())
            .header("ETag", self.etag)
            .header("Last-Modified", self.last_modified)
            .status(StatusCode::OK)
            .body(Body::from(self.data))
            .unwrap()
            .into_response()
    }
}

#[derive(Debug)]
pub struct Headers {
    pub if_none_match: Option<String>,
    pub if_modified_since: Option<String>,
}
