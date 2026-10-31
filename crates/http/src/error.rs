use std::fmt::Display;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use kellnr_common::original_name::NameError;
use kellnr_common::version::VersionError;
use kellnr_db::error::DbError;
use kellnr_storage::storage_error::StorageError;
use serde::{Deserialize, Serialize};
use zip::result::ZipError;

pub type ApiResult<T> = Result<T, ApiError>;

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
    /// An error with `msg` as its detail.
    pub fn new(msg: &str, status: StatusCode) -> Self {
        Self::from_str(msg, status)
    }

    fn from_str(e: &str, status: StatusCode) -> Self {
        Self {
            status,
            details: ErrorDetails::from(format!("ERROR: {e}")),
        }
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// The same error with another status code.
    #[must_use]
    pub fn with_status(self, status: StatusCode) -> Self {
        Self { status, ..self }
    }

    pub fn from_err(e: &dyn std::error::Error, status: StatusCode) -> Self {
        let e = format!("ERROR: {e}");
        Self {
            status,
            details: ErrorDetails::from(e),
        }
    }

    /// Log `e` server-side and return a generic 500 error that does not leak
    /// internal details like SQL statements or storage paths to the client.
    pub fn internal(e: &dyn std::error::Error) -> Self {
        tracing::error!("Internal server error: {e}");
        Self::from_str("Internal server error", StatusCode::INTERNAL_SERVER_ERROR)
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
        ApiError::internal(&e)
    }
}

impl From<ZipError> for ApiError {
    fn from(e: ZipError) -> Self {
        match e {
            ZipError::Io(e) => ApiError::internal(&e),
            ZipError::InvalidArchive(s) => ApiError::from_str(&s, StatusCode::BAD_REQUEST),
            ZipError::UnsupportedArchive(s) => ApiError::from_str(s, StatusCode::BAD_REQUEST),
            ZipError::FileNotFound => {
                ApiError::from_str("Zip archive not found", StatusCode::NOT_FOUND)
            }
            _ => ApiError::from_str("Unknown zip error", StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.details.errors[0].detail)
    }
}

impl From<DbError> for ApiError {
    fn from(e: DbError) -> Self {
        // These only carry user supplied names and are useful to the client.
        match e {
            DbError::NotOwner(_) => ApiError::from_err(&e, StatusCode::FORBIDDEN),
            DbError::UserNotFound(_)
            | DbError::GroupNotFound(_)
            | DbError::OwnerNotFound(_)
            | DbError::CrateNotFound(_)
            | DbError::CrateNotFoundWithId(_)
            | DbError::CrateMetaNotFound(_, _)
            | DbError::CrateIndexNotFound(_, _)
            | DbError::TokenNotFound
            | DbError::SessionNotFound
            | DbError::WebhookNotFound
            | DbError::ToolchainNotFound(_, _) => ApiError::from_err(&e, StatusCode::NOT_FOUND),
            DbError::PasswordMismatch
            | DbError::CrateNameConflict(_, _)
            | DbError::CrateNameMismatch(_, _)
            | DbError::InvalidCrateName(_)
            | DbError::InvalidVersion(_)
            | DbError::InvalidWebhookEvent(_)
            | DbError::InvalidId(_) => ApiError::from_err(&e, StatusCode::BAD_REQUEST),
            DbError::CrateVersionExists(_, _) | DbError::LastOwner => {
                ApiError::from_err(&e, StatusCode::CONFLICT)
            }
            _ if e.is_unique_violation() => {
                tracing::warn!("Conflict with an existing entry: {e}");
                ApiError::from_str("The entry already exists", StatusCode::CONFLICT)
            }
            _ => ApiError::internal(&e),
        }
    }
}

impl From<StorageError> for ApiError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::CrateExists(_, _) | StorageError::ToolchainArchiveExists { .. } => {
                ApiError::from_err(&e, StatusCode::CONFLICT)
            }
            _ => ApiError::internal(&e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_prefixes_the_message() {
        let e = ApiError::new("Unauthorized", StatusCode::UNAUTHORIZED);

        assert_eq!(StatusCode::UNAUTHORIZED, e.status);
        assert_eq!(1, e.details.errors.len());
        assert_eq!("ERROR: Unauthorized", e.to_string());
    }

    #[test]
    fn database_errors_are_not_leaked() {
        let e = ApiError::from(DbError::PostgresError(sea_orm::DbErr::Custom(
            "SELECT secret FROM users".to_string(),
        )));

        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, e.status);
        assert_eq!("ERROR: Internal server error", e.to_string());
    }

    #[test]
    fn user_facing_database_errors_stay_descriptive() {
        let e = ApiError::from(DbError::UserNotFound("bob".to_string()));

        assert_eq!("ERROR: User not found: bob", e.to_string());
    }

    #[test]
    fn missing_entries_are_not_found() {
        for e in [
            DbError::CrateNotFound("foo".to_string()),
            DbError::OwnerNotFound("bob".to_string()),
            DbError::TokenNotFound,
            DbError::ToolchainNotFound("stable".to_string(), "x86_64".to_string()),
        ] {
            assert_eq!(StatusCode::NOT_FOUND, ApiError::from(e).status);
        }
    }

    #[test]
    fn invalid_input_is_a_bad_request() {
        for e in [
            DbError::InvalidCrateName("f o o".to_string()),
            DbError::InvalidVersion("x".to_string()),
            DbError::InvalidId("x".to_string()),
            DbError::PasswordMismatch,
        ] {
            assert_eq!(StatusCode::BAD_REQUEST, ApiError::from(e).status);
        }
    }

    #[test]
    fn storage_and_io_errors_are_not_leaked() {
        let storage = ApiError::from(StorageError::GenericError("/var/lib/kellnr".to_string()));
        let io = ApiError::from(std::io::Error::other("/var/lib/kellnr"));

        assert_eq!("ERROR: Internal server error", storage.to_string());
        assert_eq!("ERROR: Internal server error", io.to_string());
    }

    #[test]
    fn crate_exists_storage_error_stays_descriptive() {
        let e = ApiError::from(StorageError::CrateExists(
            "foo".to_string(),
            "1.0.0".to_string(),
        ));

        assert_eq!(StatusCode::CONFLICT, e.status);
        assert_eq!(
            "ERROR: Crate with version already exists: foo-1.0.0",
            e.to_string()
        );
    }
}
