use axum::extract::Json;
use axum::{Router, routing::post};

use crate::auth::Claims;
use crate::endpoints::State;
use crate::error::ServerResponse;
use crate::forms;
use crate::persistence;

pub fn router() -> Router<State> {
    Router::new()
        .route("/exercises/unique", post(get_unique_exercises))
        .route("/exercises/statistics", post(get_exercise_statistics))
        .route("/exercises/rename", post(rename))
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct Payload {
    pub variant: forms::ExerciseVariant,
}

pub async fn get_unique_exercises(
    claims: Claims,
    axum::extract::State(State { pool }): axum::extract::State<State>,
    Json(data): Json<Payload>,
) -> ServerResponse<Json<Vec<String>>> {
    tracing::info!("Requesting unique structured exercises");

    let names = persistence::exercises::fetch_unique(claims.id, data.variant, &pool).await?;

    tracing::debug!(count = %names.len(), "Queried some unique exericses");

    Ok(Json(names))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExerciseStatistics {
    estimated_maxes: Vec<persistence::statistics::EstimatedMaxRecord>,
    rep_personal_bests: Vec<persistence::statistics::RepPersonalBest>,
}

#[derive(Debug, Deserialize)]
pub struct ExerciseStatisticsPayload {
    variant: forms::ExerciseVariant,
    description: String,
}

pub async fn get_exercise_statistics(
    claims: Claims,
    axum::extract::State(State { pool }): axum::extract::State<State>,
    Json(data): Json<ExerciseStatisticsPayload>,
) -> ServerResponse<Json<ExerciseStatistics>> {
    // Overriding the nullability here is fine as we constrain `rpe` to be non-null
    let estimated_maxes = persistence::statistics::get_estimated_maxes(
        claims.id,
        data.variant,
        &data.description,
        &pool,
    )
    .await?;

    let rep_personal_bests = persistence::statistics::get_rep_personal_bests(
        claims.id,
        data.variant,
        &data.description,
        &pool,
    )
    .await?;

    let statistics = ExerciseStatistics {
        estimated_maxes,
        rep_personal_bests,
    };

    Ok(Json(statistics))
}

#[derive(Debug, Deserialize)]
struct ExerciseRenamePayload {
    variant: forms::ExerciseVariant,
    description: String,
    updated: String,
}

#[derive(Debug, Serialize)]
struct ExerciseRenameResponse {
    rows_modified: u64,
}

async fn rename(
    claims: Claims,
    axum::extract::State(State { pool }): axum::extract::State<State>,
    Json(data): Json<ExerciseRenamePayload>,
) -> ServerResponse<Json<ExerciseRenameResponse>> {
    tracing::info!(?data, "Renaming an exercise");

    let rows_modified = persistence::exercises::rename(
        claims.id,
        data.variant,
        &data.description,
        &data.updated,
        &pool,
    )
    .await?;

    Ok(Json(ExerciseRenameResponse { rows_modified }))
}
