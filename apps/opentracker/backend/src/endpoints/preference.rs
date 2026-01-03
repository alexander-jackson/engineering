use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::endpoints::AppState;
use crate::{
    auth::Claims,
    error::ServerResponse,
    persistence::{self, preferences::Preferences},
};

pub fn router() -> Router<AppState> {
    Router::new().route("/preferences", get(fetch).put(update))
}

pub async fn fetch(
    claims: Claims,
    State(AppState { pool, .. }): State<AppState>,
) -> ServerResponse<Json<Option<Preferences>>> {
    let preferences = persistence::preferences::fetch(claims.id, &pool).await?;

    tracing::info!(?preferences, "Found some preferences for a user");

    Ok(Json(preferences))
}

pub async fn update(
    claims: Claims,
    State(AppState { pool, .. }): State<AppState>,
    Json(preferences): Json<Preferences>,
) -> ServerResponse<()> {
    persistence::preferences::update(claims.id, preferences, &pool).await?;

    tracing::info!(?preferences, "Updated preferences for a user");

    Ok(())
}
