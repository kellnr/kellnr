use axum::{response::{Response, IntoResponse}, http::StatusCode};

pub enum RouteError {
    DbError(db::error::DbError),
    InsufficientPrivileges,
    Status(StatusCode),
}

impl From<db::error::DbError> for RouteError {
    fn from(err: db::error::DbError) -> Self {
        Self::DbError(err)
    }
}

impl IntoResponse for RouteError {
    fn into_response(self) -> Response {
        match self {
            RouteError::DbError(err) => {
                tracing::error!("Db: {err:?}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            RouteError::Status(status) => status.into_response(),
            RouteError::InsufficientPrivileges => StatusCode::FORBIDDEN.into_response(),
        }
    }
}

