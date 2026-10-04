use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use color_eyre::Report;

pub type ServerResult<T> = std::result::Result<T, ServerError>;

#[derive(Debug)]
pub enum ServerError {
    NotFound,
    Internal(Report),
}

impl From<Report> for ServerError {
    fn from(value: Report) -> Self {
        Self::Internal(value)
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound => (StatusCode::NOT_FOUND, "not found").into_response(),
            Self::Internal(report) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{report:?}")).into_response()
            }
        }
    }
}
