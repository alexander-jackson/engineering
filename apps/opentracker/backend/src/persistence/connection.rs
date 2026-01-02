use axum::Extension;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sqlx::{PgPool, Postgres, pool::PoolConnection};

use crate::error::ServerError;

pub type Connection = PoolConnection<Postgres>;
pub struct ConnectionExtractor(pub Connection);

impl<State> FromRequestParts<State> for ConnectionExtractor
where
    State: Send + Sync,
{
    type Rejection = ServerError;

    async fn from_request_parts(parts: &mut Parts, state: &State) -> Result<Self, Self::Rejection> {
        let Extension(pool) = Extension::<PgPool>::from_request_parts(parts, state)
            .await
            .map_err(|_| ServerError::INTERNAL_SERVER_ERROR)?;

        let conn = pool
            .acquire()
            .await
            .map_err(|_| ServerError::INTERNAL_SERVER_ERROR)?;

        Ok(Self(conn))
    }
}
