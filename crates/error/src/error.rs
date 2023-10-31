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

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiError {
    pub errors: Vec<ErrorDetails>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorDetails {
    pub detail: String,
}

impl ApiError {
    pub fn new(msg: &str, error: &dyn ToString) -> Self {
        let error = error.to_string();
        let e = if error.is_empty() {
            format!("ERROR: {}", msg)
        } else {
            format!("ERROR: {} -> {}", msg, error)
        };
        let detail = ErrorDetails { detail: e };
        Self {
            errors: vec![detail],
        }
    }

    fn from_dyn_str(e: &dyn ToString) -> Self {
        let e = format!("ERROR: {}", e.to_string());
        let detail = ErrorDetails { detail: e };
        Self {
            errors: vec![detail],
        }
    }

    fn from_str(msg: &str) -> Self {
        let msg = format!("ERROR: {}", msg);
        let detail = ErrorDetails { detail: msg };
        Self {
            errors: vec![detail],
        }
    }

    pub fn not_owner() -> Self {
        Self::from_str("Not an owner of the crate.")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

impl From<NameError> for ApiError {
    fn from(name_error: NameError) -> Self {
        ApiError::from_str(&name_error.to_string())
    }
}

impl From<VersionError> for ApiError {
    fn from(version_error: VersionError) -> Self {
        ApiError::from_str(&version_error.to_string())
    }
}

impl From<&String> for ApiError {
    fn from(e: &String) -> Self {
        ApiError::from_str(e)
    }
}

impl From<&str> for ApiError {
    fn from(e: &str) -> Self {
        ApiError::from_str(e)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::from_dyn_str(&e)
    }
}

impl From<&dyn ToString> for ApiError {
    fn from(e: &dyn ToString) -> Self {
        ApiError::from_dyn_str(e)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::from_dyn_str(&e)
    }
}

impl From<&anyhow::Error> for ApiError {
    fn from(e: &anyhow::Error) -> Self {
        ApiError::from_dyn_str(e)
    }
}

impl From<zip::result::ZipError> for ApiError {
    fn from(e: zip::result::ZipError) -> Self {
        match e {
            ZipError::Io(e) => ApiError::from_dyn_str(&e),
            ZipError::InvalidArchive(s) => ApiError::from_str(s),
            ZipError::UnsupportedArchive(s) => ApiError::from_str(s),
            ZipError::FileNotFound => ApiError::from_str("File not found"),
        }
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.errors[0].detail)
    }
}

impl From<db::error::DbError> for ApiError {
    fn from(e: db::error::DbError) -> Self {
        ApiError::from_str(&e.to_string())
    }
}
