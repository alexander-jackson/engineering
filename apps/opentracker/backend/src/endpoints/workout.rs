use axum::extract::{Json, Path, Query};
use axum::routing::{get, Router};

use crate::auth::Claims;
use crate::error::ServerResponse;
use crate::forms;
use crate::persistence::{self, ConnectionExtractor};
use crate::utils;

#[derive(Debug, Deserialize)]
pub struct DateRange {
    start: chrono::DateTime<chrono::Utc>,
    end: chrono::DateTime<chrono::Utc>,
}

pub fn router() -> Router {
    Router::new()
        .route("/workouts", get(get_workouts))
        .route(
            "/workouts/:recorded",
            get(get_workout).put(upload_workout).delete(delete_workout),
        )
        .route("/workouts/statistics", get(get_workout_statistics))
}

pub async fn get_workouts(
    ConnectionExtractor(mut conn): ConnectionExtractor,
    claims: Claims,
    Query(range): Query<DateRange>,
) -> ServerResponse<Json<Vec<persistence::workouts::DatedWorkout>>> {
    let dated_exercises = persistence::workouts::fetch_with_exercises_between(
        claims.id,
        range.start,
        range.end,
        &mut conn,
    )
    .await?;

    let grouped = utils::group_by_date(dated_exercises);

    tracing::info!(count = %grouped.len(), ?range, "Queried a user's workouts");

    Ok(Json(grouped))
}

pub async fn get_workout(
    ConnectionExtractor(mut conn): ConnectionExtractor,
    claims: Claims,
    Path(recorded): Path<forms::RecordedDate>,
) -> ServerResponse<Json<Vec<forms::Exercise>>> {
    // Fetch all the exercises for the workout
    let exercises =
        persistence::workouts::fetch_exercises_for_workout(claims.id, recorded.0, &mut conn)
            .await?;

    tracing::info!(?recorded, ?exercises, "Queried a specific workout");

    Ok(Json(exercises))
}

pub async fn delete_workout(
    ConnectionExtractor(mut conn): ConnectionExtractor,
    claims: Claims,
    Path(recorded): Path<forms::RecordedDate>,
) -> ServerResponse<()> {
    // Delete the workout entry itself
    persistence::workouts::delete_by_date(claims.id, recorded.0, &mut conn).await?;

    tracing::info!(?recorded, "Deleted a specific workout");

    Ok(())
}

pub async fn upload_workout(
    ConnectionExtractor(mut conn): ConnectionExtractor,
    claims: Claims,
    Path(recorded): Path<forms::RecordedDate>,
    Json(data): Json<forms::Workout>,
) -> ServerResponse<()> {
    // Delete the exercises for this date
    persistence::exercises::delete_by_date(claims.id, recorded.0, &mut conn).await?;

    // Get the ID of the workout, either by creating a new one or getting the existing one
    let workout_id =
        persistence::workouts::create_or_fetch(claims.id, recorded.0, &mut conn).await?;

    // Insert each of the exercises based on the returned identifier
    for exercise in &data.exercises {
        persistence::exercises::insert(workout_id, exercise, &mut conn).await?;
    }

    tracing::info!(?workout_id, "Inserted/updated a workout in the new format");

    Ok(())
}

#[derive(Deserialize)]
pub struct WorkoutStatisticsQuery {
    end: chrono::DateTime<chrono::Utc>,
}

pub async fn get_workout_statistics(
    ConnectionExtractor(mut conn): ConnectionExtractor,
    claims: Claims,
    Query(query): Query<WorkoutStatisticsQuery>,
) -> ServerResponse<Json<persistence::statistics::WorkoutStatistics>> {
    let date = query.end.date_naive();
    let stats = persistence::statistics::get_workout_statistics(claims.id, date, &mut conn).await?;

    tracing::info!("Fetched workout statistics");

    Ok(Json(stats))
}
