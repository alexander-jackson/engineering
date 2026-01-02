use axum::http::StatusCode;
use axum::response::IntoResponse;

pub type ServerResponse<T> = Result<T, ServerError>;

#[derive(Copy, Clone, Debug, Serialize)]
struct ErrorBody {
    message: &'static str,
}

impl ErrorBody {
    pub fn new(message: &'static str) -> Self {
        Self { message }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ServerError {
    status: StatusCode,
    body: Option<ErrorBody>,
}

impl ServerError {
    pub const UNAUTHORIZED: Self = Self::new(StatusCode::UNAUTHORIZED);
    pub const NOT_FOUND: Self = Self::new(StatusCode::NOT_FOUND);
    pub const CONFLICT: Self = Self::new(StatusCode::CONFLICT);
    pub const UNPROCESSABLE_ENTITY: Self = Self::new(StatusCode::UNPROCESSABLE_ENTITY);
    pub const INTERNAL_SERVER_ERROR: Self = Self::new(StatusCode::INTERNAL_SERVER_ERROR);

    pub const fn new(status: StatusCode) -> Self {
        Self { status, body: None }
    }

    pub fn with_message(&self, message: &'static str) -> Self {
        Self {
            status: self.status,
            body: Some(ErrorBody::new(message)),
        }
    }
}

impl From<sqlx::Error> for ServerError {
    fn from(_: sqlx::Error) -> Self {
        Self::INTERNAL_SERVER_ERROR
    }
}

impl From<jsonwebtoken::errors::Error> for ServerError {
    fn from(_: jsonwebtoken::errors::Error) -> Self {
        Self::UNPROCESSABLE_ENTITY
    }
}

impl From<bcrypt::BcryptError> for ServerError {
    fn from(_: bcrypt::BcryptError) -> Self {
        Self::UNPROCESSABLE_ENTITY
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        let serialized = self
            .body
            .map(|b| serde_json::to_string(&b).unwrap())
            .unwrap_or_default();

        (self.status, serialized).into_response()
    }
}
