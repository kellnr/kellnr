use error::api_error::ApiError;
use hyper::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DocsError {
    #[error("Failed to extract docs")]
    ExtractFailed,
    #[error("Crate with version does not exist: {0}-{1}")]
    CrateDoesNotExist(String, String),
}

impl From<DocsError> for ApiError {
    fn from(e: DocsError) -> Self {
        match e {
            DocsError::ExtractFailed => ApiError::from_err(&e, StatusCode::INTERNAL_SERVER_ERROR),
            DocsError::CrateDoesNotExist(_, _) => ApiError::from_err(&e, StatusCode::NOT_FOUND),
        }
    }
}
