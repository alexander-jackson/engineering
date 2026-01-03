use axum::routing::get;
use axum::{Json, Router};

use crate::endpoints::State;
use crate::{
    auth::Claims,
    error::ServerResponse,
    persistence::{self, preferences::Preferences},
};

pub fn router(state: State) -> Router {
    Router::new()
        .route("/preferences", get(fetch).put(update))
        .with_state(state)
}

pub async fn fetch(
    claims: Claims,
    axum::extract::State(State { pool }): axum::extract::State<State>,
) -> ServerResponse<Json<Option<Preferences>>> {
    let preferences = persistence::preferences::fetch(claims.id, &pool).await?;

    tracing::info!(?preferences, "Found some preferences for a user");

    Ok(Json(preferences))
}

pub async fn update(
    claims: Claims,
    axum::extract::State(State { pool }): axum::extract::State<State>,
    Json(preferences): Json<Preferences>,
) -> ServerResponse<()> {
    persistence::preferences::update(claims.id, preferences, &pool).await?;

    tracing::info!(?preferences, "Updated preferences for a user");

    Ok(())
}
