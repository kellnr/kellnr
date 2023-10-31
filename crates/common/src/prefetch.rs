use axum::{
    body::Full,
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
            .body(Full::from(self.data))
            .unwrap()
            .into_response()
    }
}

#[derive(Debug)]
pub struct Headers {
    pub if_none_match: Option<String>,
    pub if_modified_since: Option<String>,
}

// #[rocket::async_trait]
// impl<'r> FromRequest<'r> for Headers {
//     type Error = String;
//
//     async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
//         let if_modified_since = request
//             .headers()
//             .get_one("if-modified-since")
//             .map(|h| h.to_string());
//         let if_none_match = request
//             .headers()
//             .get_one("if-none-match")
//             .map(|h| h.to_string());
//
//         Outcome::Success(Headers {
//             if_none_match,
//             if_modified_since,
//         })
//     }
// }
