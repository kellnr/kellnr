use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use kellnr_common::account_name::AccountNameError;
use kellnr_db::error::DbError;

#[derive(Debug)]
pub enum RouteError {
    DbError(DbError),
    InsufficientPrivileges,
    Status(StatusCode),
    AuthenticationFailure,
    /// The requested entry does not exist (404).
    NotFound(String),
    /// The request is invalid, the message tells the client why (400).
    BadRequest(String),
    /// The request conflicts with an existing entry (409).
    Conflict(String),
}

impl From<DbError> for RouteError {
    fn from(err: DbError) -> Self {
        match err {
            DbError::PasswordMismatch => Self::AuthenticationFailure,
            DbError::NotOwner(_) => Self::InsufficientPrivileges,
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
            | DbError::ToolchainNotFound(_, _) => Self::NotFound(err.to_string()),
            DbError::CrateNameConflict(_, _)
            | DbError::CrateNameMismatch(_, _)
            | DbError::InvalidCrateName(_)
            | DbError::InvalidVersion(_)
            | DbError::InvalidWebhookEvent(_)
            | DbError::InvalidId(_) => Self::BadRequest(err.to_string()),
            DbError::CrateVersionExists(_, _) | DbError::LastOwner => {
                Self::Conflict(err.to_string())
            }
            _ if err.is_unique_violation() => {
                tracing::warn!("Conflict with an existing entry: {err}");
                Self::Conflict("The entry already exists".to_string())
            }
            _ => Self::DbError(err),
        }
    }
}

impl From<AccountNameError> for RouteError {
    fn from(err: AccountNameError) -> Self {
        Self::BadRequest(err.to_string())
    }
}

impl IntoResponse for RouteError {
    fn into_response(self) -> Response {
        match self {
            Self::AuthenticationFailure => {
                tracing::warn!("Login with wrong username or password");
                StatusCode::UNAUTHORIZED.into_response()
            }
            RouteError::DbError(err) => {
                tracing::error!("Db: {err:?}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            RouteError::Status(status) => status.into_response(),
            RouteError::InsufficientPrivileges => StatusCode::FORBIDDEN.into_response(),
            RouteError::NotFound(msg) => {
                tracing::warn!("{msg}");
                (StatusCode::NOT_FOUND, msg).into_response()
            }
            RouteError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            RouteError::Conflict(msg) => (StatusCode::CONFLICT, msg).into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(e: DbError) -> StatusCode {
        RouteError::from(e).into_response().status()
    }

    #[test]
    fn missing_entries_are_not_found() {
        for e in [
            DbError::UserNotFound("bob".to_string()),
            DbError::GroupNotFound("devs".to_string()),
            DbError::CrateNotFound("foo".to_string()),
            DbError::TokenNotFound,
        ] {
            assert_eq!(status(e), StatusCode::NOT_FOUND);
        }
    }

    #[test]
    fn invalid_input_is_a_bad_request() {
        assert_eq!(
            status(DbError::InvalidCrateName("-".to_string())),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn existing_entries_are_a_conflict() {
        assert_eq!(status(DbError::LastOwner), StatusCode::CONFLICT);
    }

    #[test]
    fn other_database_errors_are_internal() {
        assert_eq!(
            status(DbError::FailedToCountCrates),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
