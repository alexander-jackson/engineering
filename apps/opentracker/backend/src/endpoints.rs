use axum::extract::FromRef;
use axum::http::Method;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::routing::Router;
use jsonwebtoken::DecodingKey;
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};

use crate::error::ServerResponse;

pub mod bodyweight;
pub mod exercise;
pub mod preference;
pub mod user;
pub mod workout;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub decoding_key: DecodingKey,
}

impl FromRef<AppState> for DecodingKey {
    fn from_ref(state: &AppState) -> Self {
        state.decoding_key.clone()
    }
}

pub async fn health() -> ServerResponse<&'static str> {
    tracing::info!("Responding as healthy to an incoming request");

    Ok("Server is healthy 👋")
}

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_origin(Any);

    Router::new()
        .merge(bodyweight::router())
        .merge(exercise::router())
        .merge(preference::router())
        .merge(user::router())
        .merge(workout::router())
        .layer(cors)
        .with_state(state)
}
