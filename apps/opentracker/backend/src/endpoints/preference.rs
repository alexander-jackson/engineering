use axum::routing::get;
use axum::{Json, Router};

use crate::{
    auth::Claims,
    error::ServerResponse,
    persistence::{self, preferences::Preferences, ConnectionExtractor},
};

pub fn router() -> Router {
    Router::new().route("/preferences", get(fetch).put(update))
}

pub async fn fetch(
    claims: Claims,
    ConnectionExtractor(mut conn): ConnectionExtractor,
) -> ServerResponse<Json<Option<Preferences>>> {
    let preferences = persistence::preferences::fetch(claims.id, &mut conn).await?;

    tracing::info!(?preferences, "Found some preferences for a user");

    Ok(Json(preferences))
}

pub async fn update(
    claims: Claims,
    ConnectionExtractor(mut conn): ConnectionExtractor,
    Json(preferences): Json<Preferences>,
) -> ServerResponse<()> {
    persistence::preferences::update(claims.id, preferences, &mut conn).await?;

    tracing::info!(?preferences, "Updated preferences for a user");

    Ok(())
}
