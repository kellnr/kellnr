use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use common::original_name::NameError;
use common::version::VersionError;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use zip::result::ZipError;

pub type ApiResult<T> = core::result::Result<T, ApiError>;

pub struct ApiError {
    status: StatusCode,
    details: ErrorDetails,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorDetails {
    pub errors: Vec<ErrorDetail>,
}

impl From<String> for ErrorDetails {
    fn from(e: String) -> Self {
        let detail = ErrorDetail { detail: e };
        Self {
            errors: vec![detail],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorDetail {
    pub detail: String,
}

impl ApiError {
    pub fn new(msg: &str, error: &dyn ToString, status: StatusCode) -> Self {
        let error = error.to_string();
        let e = if error.is_empty() {
            format!("ERROR: {}", msg)
        } else {
            format!("ERROR: {} -> {}", msg, error)
        };
        Self {
            status,
            details: ErrorDetails::from(e),
        }
    }

    fn from_dyn_str(e: &dyn ToString, status: StatusCode) -> Self {
        let e = format!("ERROR: {}", e.to_string());
        Self {
            status,
            details: ErrorDetails::from(e),
        }
    }

    pub fn from_err(e: &dyn std::error::Error, status: StatusCode) -> Self {
        let e = format!("ERROR: {}", e.to_string());
        Self {
            status,
            details: ErrorDetails::from(e),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.details)).into_response()
    }
}

impl From<NameError> for ApiError {
    fn from(e: NameError) -> Self {
       ApiError::from_err(&e, StatusCode::BAD_REQUEST) 
    }
}

impl From<VersionError> for ApiError {
    fn from(e: VersionError) -> Self {
        ApiError::from_err(&e, StatusCode::BAD_REQUEST)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::from_err(&e, StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl From<zip::result::ZipError> for ApiError {
    fn from(e: zip::result::ZipError) -> Self {
        match e {
            ZipError::Io(e) => ApiError::from_err(&e, StatusCode::INTERNAL_SERVER_ERROR),
            ZipError::InvalidArchive(s) => {
                ApiError::from_dyn_str(&s.to_string(), StatusCode::BAD_REQUEST)
            }
            ZipError::UnsupportedArchive(s) => {
                ApiError::from_dyn_str(&s.to_string(), StatusCode::BAD_REQUEST)
            }
            ZipError::FileNotFound => ApiError::from_dyn_str(
                &String::from("Zip archive not found"),
                StatusCode::NOT_FOUND,
            ),
        }
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.details.errors[0].detail)
    }
}

impl From<db::error::DbError> for ApiError {
    fn from(e: db::error::DbError) -> Self {
        ApiError::from_err(&e, StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl From<storage::storage_error::StorageError> for ApiError {
    fn from(e: storage::storage_error::StorageError) -> Self {
        ApiError::from_err(&e, StatusCode::INTERNAL_SERVER_ERROR)
    }
}
