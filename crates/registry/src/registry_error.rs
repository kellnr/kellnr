use hyper::StatusCode;
use kellnr_common::publish_metadata::InvalidUrl;
use kellnr_http::error::ApiError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("Invalid metadata string: {0}")]
    InvalidMetadataString(#[from] serde_json::error::Error),
    #[error("Invalid metadata length: {0}")]
    InvalidMetadataLength(#[from] std::array::TryFromSliceError),
    #[error("Invalid metadata size")]
    InvalidMetadataSize,
    #[error("Unexpected {0} bytes after the crate data")]
    TrailingData(usize),
    #[error("Invalid min. length {0}/{1} bytes")]
    InvalidMinLength(usize, usize),
    #[error("Body of {0} bytes exceeds the maximum of {1} bytes")]
    TooLarge(usize, usize),
    #[error("Failed to parse URL: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("Failed request: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Crates.io responded with {0}")]
    UpstreamStatus(StatusCode),
    #[error("Crate with version already exists: {0}-{1}")]
    CrateExists(String, String),
    #[error("Failed to extract bytes from request: {0}")]
    ExtractBytesFailed(#[from] axum::extract::rejection::BytesRejection),
    #[error(
        "A different crate file of {0} {1} exists in the storage without being published. An administrator must remove the file {0}-{1}.crate from the crate storage"
    )]
    OrphanedCrateFile(String, String),
    #[error("Crate not found")]
    CrateNotFound,
    #[error("Required field(s) not defined for crate {0}, missing: {1:?}, requires: {2:?}")]
    MissingRequiredFields(String, Vec<String>, Vec<String>),
    #[error("New crates publishing has been restricted")]
    NewCratesRestricted,
    #[error("{0}")]
    InvalidUrl(#[from] InvalidUrl),
}

impl From<RegistryError> for ApiError {
    fn from(e: RegistryError) -> Self {
        match e {
            // E.g. 413 if the body is larger than the configured maximum.
            RegistryError::ExtractBytesFailed(ref rejection) => {
                let status = rejection.status();
                ApiError::from_err(&e, status)
            }
            RegistryError::CrateNotFound => ApiError::from_err(&e, StatusCode::NOT_FOUND),
            RegistryError::TooLarge(_, _) => ApiError::from_err(&e, StatusCode::PAYLOAD_TOO_LARGE),
            RegistryError::RequestError(_) | RegistryError::UpstreamStatus(_) => {
                ApiError::from_err(&e, StatusCode::BAD_GATEWAY)
            }
            RegistryError::CrateExists(_, _) | RegistryError::OrphanedCrateFile(_, _) => {
                ApiError::from_err(&e, StatusCode::CONFLICT)
            }
            _ => ApiError::from_err(&e, StatusCode::BAD_REQUEST),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::{Body, Bytes};
    use axum::extract::DefaultBodyLimit;
    use axum::extract::rejection::BytesRejection;
    use axum::http::Request;
    use axum::response::{IntoResponse, Response};
    use axum::routing::put;
    use tower::ServiceExt;

    use super::*;

    async fn read_body(body: Result<Bytes, BytesRejection>) -> Response {
        match body {
            Ok(_) => StatusCode::OK.into_response(),
            Err(e) => ApiError::from(RegistryError::ExtractBytesFailed(e)).into_response(),
        }
    }

    #[tokio::test]
    async fn too_large_body_is_payload_too_large() {
        let app = Router::new()
            .route("/", put(read_body))
            .layer(DefaultBodyLimit::max(4));

        let response = app
            .oneshot(Request::put("/").body(Body::from(vec![0; 8])).unwrap())
            .await
            .unwrap();

        assert_eq!(StatusCode::PAYLOAD_TOO_LARGE, response.status());
    }

    #[test]
    fn existing_crate_version_is_a_conflict() {
        let e = ApiError::from(RegistryError::CrateExists(
            "foo".to_string(),
            "1.0.0".to_string(),
        ));

        assert_eq!(StatusCode::CONFLICT, e.into_response().status());
    }
}
