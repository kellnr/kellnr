use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug)]
pub enum RouteError {
    DbError(db::error::DbError),
    InsufficientPrivileges,
    Status(StatusCode),
    PasswordMissmatch,
}

impl From<db::error::DbError> for RouteError {
    fn from(err: db::error::DbError) -> Self {
        match err {
            db::error::DbError::PasswordMismatch => Self::PasswordMissmatch,
            _ => Self::DbError(err),
        }
    }
}

impl IntoResponse for RouteError {
    fn into_response(self) -> Response {
        match self {
            Self::PasswordMissmatch => { 
                tracing::warn!("Login with wrong username or password");
                StatusCode::UNAUTHORIZED.into_response()
                },
            RouteError::DbError(err) => {
                tracing::error!("Db: {err:?}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            RouteError::Status(status) => status.into_response(),
            RouteError::InsufficientPrivileges => StatusCode::FORBIDDEN.into_response(),
        }
    }
}
