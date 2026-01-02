use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug)]
pub enum RouteError {
    DbError(kellnr_db::error::DbError),
    InsufficientPrivileges,
    Status(StatusCode),
    AuthenticationFailure,
    UserNotFound(String),
}

impl From<kellnr_db::error::DbError> for RouteError {
    fn from(err: kellnr_db::error::DbError) -> Self {
        match err {
            kellnr_db::error::DbError::PasswordMismatch => Self::AuthenticationFailure,
            _ => Self::DbError(err),
        }
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
            RouteError::UserNotFound(name) => {
                tracing::warn!("User not found: {name}");
                StatusCode::NOT_FOUND.into_response()
            }
        }
    }
}
