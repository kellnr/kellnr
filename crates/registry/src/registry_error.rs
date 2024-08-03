use error::api_error::ApiError;
use hyper::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("Invalid raw metadata: {0}")]
    InvalidRawMetadata(#[from] std::string::FromUtf8Error),
    #[error("Invalid metadata string: {0}")]
    InvalidMetadataString(#[from] serde_json::error::Error),
    #[error("Invalid metadata length: {0}")]
    InvalidMetadataLength(#[from] std::array::TryFromSliceError),
    #[error("Invalid metadata size")]
    InvalidMetadataSize,
    #[error("Invalid min. length {0}/{1} bytes")]
    InvalidMinLength(usize, usize),
    #[error("Failed to parse URL: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("Failed request: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Crate with version already exists: {0}-{1}")]
    CrateExists(String, String),
    #[error("Failed to extract bytes from request: {0}")]
    ExtractBytesFailed(#[from] axum::extract::rejection::BytesRejection),
    #[error("Not the owner of the crate")]
    NotOwner,
    #[error("No authorization for download provided")]
    DownloadUnauthorized,
    #[error("Not a crate user")]
    NotCrateUser,
    #[error("Crate not found")]
    CrateNotFound,
}

impl From<RegistryError> for ApiError {
    fn from(e: RegistryError) -> Self {
        match e {
            RegistryError::CrateNotFound => ApiError::from_err(&e, StatusCode::NOT_FOUND),
            RegistryError::DownloadUnauthorized => ApiError::from_err(&e, StatusCode::UNAUTHORIZED),
            RegistryError::NotOwner | RegistryError::NotCrateUser => {
                ApiError::from_err(&e, StatusCode::FORBIDDEN)
            }
            _ => ApiError::from_err(&e, StatusCode::BAD_REQUEST),
        }
    }
}
