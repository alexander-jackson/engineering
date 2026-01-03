use axum::extract::{Json, Path, State};
use axum::routing::{Router, get};

use crate::auth::Claims;
use crate::endpoints::AppState;
use crate::error::{ServerError, ServerResponse};
use crate::forms;
use crate::persistence;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/bodyweights/{recorded}",
            get(get_specific_bodyweight)
                .put(upload_bodyweight)
                .delete(delete_specific_bodyweight),
        )
        .route("/bodyweights", get(get_all_bodyweights))
        .route("/bodyweights/most-recent", get(get_most_recent_bodyweight))
        .route("/bodyweights/statistics", get(get_bodyweight_statistics))
}

pub async fn get_specific_bodyweight(
    State(AppState { pool }): State<AppState>,
    claims: Claims,
    Path(recorded): Path<forms::RecordedDate>,
) -> ServerResponse<Json<persistence::bodyweights::SpecificBodyweightRecord>> {
    let contents = persistence::bodyweights::fetch_by_date(claims.id, recorded.0, &pool).await?;

    tracing::info!(?recorded, ?contents, "Queried a specific bodyweight");

    // Return a 404 if there's no data here
    contents.map(Json).ok_or(ServerError::NOT_FOUND)
}

pub async fn delete_specific_bodyweight(
    State(AppState { pool }): State<AppState>,
    claims: Claims,
    Path(recorded): Path<forms::RecordedDate>,
) -> ServerResponse<()> {
    persistence::bodyweights::delete_by_date(claims.id, recorded.0, &pool).await?;

    tracing::info!(?recorded, "Deleted a specific bodyweight");

    Ok(())
}

#[derive(Serialize)]
pub struct BodyweightRecords {
    labels: Vec<forms::RecordedDate>,
    values: Vec<f32>,
}

pub async fn get_all_bodyweights(
    State(AppState { pool }): State<AppState>,
    claims: Claims,
) -> ServerResponse<Json<BodyweightRecords>> {
    let bodyweights = persistence::bodyweights::fetch_all(claims.id, &pool).await?;

    tracing::info!(count = %bodyweights.len(), "Queried all bodyweights");

    let items = bodyweights.len();

    let mut labels = Vec::with_capacity(items);
    let mut values = Vec::with_capacity(items);

    bodyweights.iter().for_each(|x| {
        labels.push(x.recorded);
        values.push(x.bodyweight);
    });

    let response = BodyweightRecords { labels, values };

    Ok(Json(response))
}

pub async fn upload_bodyweight(
    State(AppState { pool }): State<AppState>,
    claims: Claims,
    Path(recorded): Path<forms::RecordedDate>,
    Json(data): Json<forms::Bodyweight>,
) -> ServerResponse<()> {
    persistence::bodyweights::insert(claims.id, data.bodyweight, recorded.0, &pool).await?;

    tracing::info!(?data, "Inserted/updated a bodyweight record");

    Ok(())
}

pub async fn get_most_recent_bodyweight(
    State(AppState { pool }): State<AppState>,
    claims: Claims,
) -> ServerResponse<Json<Option<persistence::bodyweights::BodyweightRecord>>> {
    let most_recent_bodyweight =
        persistence::bodyweights::fetch_most_recent(claims.id, &pool).await?;

    tracing::info!("Fetched most recent bodyweight");

    Ok(Json(most_recent_bodyweight))
}

pub async fn get_bodyweight_statistics(
    State(AppState { pool }): State<AppState>,
    claims: Claims,
) -> ServerResponse<Json<persistence::statistics::BodyweightStatistics>> {
    let stats = persistence::statistics::get_bodyweight_statistics(claims.id, &pool).await?;

    tracing::info!("Fetched bodyweight statistics");

    Ok(Json(stats))
}
