use hyper::StatusCode;
use kellnr_error::api_error::ApiError;
use kellnr_storage::storage_error::StorageError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DocsError {
    #[error("Failed to extract docs")]
    ExtractFailed,
    #[error("Crate with version does not exist: {0}-{1}")]
    CrateDoesNotExist(String, String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] kellnr_db::error::DbError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to walk docs directory: {0}")]
    DirWalkError(#[from] fs_extra::error::Error),
    #[error("Cargo error: {0}")]
    CargoError(String),
    #[error("Docs storage error: {0}")]
    StorageError(#[from] StorageError),
    #[error("Path {path} is not inside {root}")]
    PathPrefixMismatch { path: String, root: String },
}

impl From<DocsError> for ApiError {
    fn from(e: DocsError) -> Self {
        match e {
            DocsError::ExtractFailed => ApiError::from_err(&e, StatusCode::INTERNAL_SERVER_ERROR),
            DocsError::CrateDoesNotExist(_, _) => ApiError::from_err(&e, StatusCode::NOT_FOUND),
            DocsError::DatabaseError(db_error) => {
                ApiError::from_err(&db_error, StatusCode::INTERNAL_SERVER_ERROR)
            }
            DocsError::IoError(error) => {
                ApiError::from_err(&error, StatusCode::INTERNAL_SERVER_ERROR)
            }
            DocsError::DirWalkError(error) => {
                ApiError::from_err(&error, StatusCode::INTERNAL_SERVER_ERROR)
            }
            DocsError::CargoError(error) => ApiError::new(
                &error,
                &String::default(),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            DocsError::StorageError(error) => ApiError::from(error),
            DocsError::PathPrefixMismatch { .. } => {
                ApiError::from_err(&e, StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}
