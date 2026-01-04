use axum::extract::{Json, State};
use axum::{Router, routing::post};

use crate::auth::Claims;
use crate::endpoints::AppState;
use crate::error::ServerResponse;
use crate::forms;
use crate::persistence;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/exercises/unique", post(get_unique_exercises))
        .route("/exercises/statistics", post(get_exercise_statistics))
        .route("/exercises/rename", post(rename))
        .route("/exercises/last-session", post(get_last_session))
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct Payload {
    pub variant: forms::ExerciseVariant,
}

pub async fn get_unique_exercises(
    claims: Claims,
    State(AppState { pool }): State<AppState>,
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
    State(AppState { pool }): State<AppState>,
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
    State(AppState { pool }): State<AppState>,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastSessionPayload {
    variant: forms::ExerciseVariant,
    description: String,
    current_date: String,
}

pub async fn get_last_session(
    claims: Claims,
    State(AppState { pool }): State<AppState>,
    Json(data): Json<LastSessionPayload>,
) -> ServerResponse<Json<Option<forms::LastExerciseSession>>> {
    tracing::info!(
        ?data.variant,
        ?data.description,
        "Fetching last session for exercise"
    );

    let current_date =
        chrono::NaiveDate::parse_from_str(&data.current_date, "%Y-%m-%d").map_err(|_| {
            crate::error::ServerError::UNPROCESSABLE_ENTITY.with_message("Invalid date format")
        })?;

    let last_session = persistence::exercises::fetch_last_session(
        claims.id,
        data.variant,
        &data.description,
        current_date,
        &pool,
    )
    .await?;

    let response = last_session.map(|(exercise, recorded)| forms::LastExerciseSession {
        recorded: recorded.format("%Y-%m-%d").to_string(),
        exercise,
    });

    Ok(Json(response))
}
