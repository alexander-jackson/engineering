use axum::http::{
    Method,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use axum::routing::Router;
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
