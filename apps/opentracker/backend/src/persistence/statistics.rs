use std::ops::DerefMut;

use chrono::Duration;
use uuid::Uuid;

use crate::forms;
use crate::persistence::Connection;

#[derive(Copy, Clone, Debug, PartialEq, Serialize)]
pub struct RepPersonalBest {
    pub weight: f32,
    pub reps: i32,
    pub recorded: forms::RecordedDate,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimatedMaxRecord {
    estimate: f64,
    recorded: forms::RecordedDate,
}

pub async fn get_estimated_maxes(
    user_id: Uuid,
    variant: forms::ExerciseVariant,
    description: &str,
    conn: &mut Connection,
) -> sqlx::Result<Vec<EstimatedMaxRecord>> {
    match variant {
        forms::ExerciseVariant::Other => {
            get_estimated_maxes_freeform(user_id, description, conn).await
        }
        _ => get_estimated_maxes_structured(user_id, variant, description, conn).await,
    }
}

pub async fn get_estimated_maxes_structured(
    user_id: Uuid,
    variant: forms::ExerciseVariant,
    description: &str,
    conn: &mut Connection,
) -> sqlx::Result<Vec<EstimatedMaxRecord>> {
    sqlx::query_as!(
        EstimatedMaxRecord,
        r#"
        SELECT max((100 * weight) / (48.8 + 53.8 * exp(-0.075 * (reps + (10 - rpe))))) AS "estimate!", recorded AS "recorded: forms::RecordedDate"
        FROM structured_exercise
        INNER JOIN workout ON workout.id = structured_exercise.workout_id
        WHERE workout.account_id = (SELECT id FROM account WHERE account_uid = $1)
        AND rpe IS NOT NULL
        AND variant = $2
        AND description = $3
        GROUP BY recorded
        ORDER BY recorded
        "#,
        user_id,
        variant as forms::ExerciseVariant,
        description,
    )
    .fetch_all(conn.deref_mut())
    .await
}

pub async fn get_estimated_maxes_freeform(
    user_id: Uuid,
    description: &str,
    conn: &mut Connection,
) -> sqlx::Result<Vec<EstimatedMaxRecord>> {
    sqlx::query_as!(
        EstimatedMaxRecord,
        r#"
        SELECT max((100 * weight) / (48.8 + 53.8 * exp(-0.075 * (reps + (10 - rpe))))) AS "estimate!", recorded AS "recorded: forms::RecordedDate"
        FROM freeform_exercise
        INNER JOIN workout ON workout.id = freeform_exercise.workout_id
        WHERE workout.account_id = (SELECT id FROM account WHERE account_uid = $1)
        AND rpe IS NOT NULL
        AND description = $2
        GROUP BY recorded
        ORDER BY recorded
        "#,
        user_id,
        description,
    )
    .fetch_all(conn.deref_mut())
    .await
}

pub async fn get_rep_personal_bests(
    user_id: Uuid,
    variant: forms::ExerciseVariant,
    description: &str,
    conn: &mut Connection,
) -> sqlx::Result<Vec<RepPersonalBest>> {
    match variant {
        forms::ExerciseVariant::Other => {
            get_freeform_rep_personal_bests(user_id, description, conn).await
        }
        _ => get_structured_rep_personal_bests(user_id, variant, description, conn).await,
    }
}

pub async fn get_structured_rep_personal_bests(
    user_id: Uuid,
    variant: forms::ExerciseVariant,
    description: &str,
    conn: &mut Connection,
) -> sqlx::Result<Vec<RepPersonalBest>> {
    sqlx::query_as!(
        RepPersonalBest,
        r#"
        WITH rep_pbs AS (
            SELECT MAX(weight) AS weight, reps
            FROM structured_exercise se
            JOIN workout w ON se.workout_id = w.id
            JOIN account a ON w.account_id = a.id
            WHERE a.account_uid = $1
            AND se.variant = $2
            AND se.description = $3
            GROUP BY reps
        )
        SELECT DISTINCT ON (rp.weight, rp.reps)
            rp.weight AS "weight!",
            rp.reps,
            w.recorded AS "recorded: forms::RecordedDate"
        FROM rep_pbs rp
        JOIN structured_exercise se ON (rp.weight = se.weight AND rp.reps = se.reps)
        JOIN workout w ON se.workout_id = w.id
        JOIN account a ON w.account_id = a.id
        WHERE a.account_uid = $1
        AND se.variant = $2
        AND se.description = $3
        ORDER BY reps
        "#,
        user_id,
        variant as forms::ExerciseVariant,
        description,
    )
    .fetch_all(conn.deref_mut())
    .await
}

pub async fn get_freeform_rep_personal_bests(
    user_id: Uuid,
    description: &str,
    conn: &mut Connection,
) -> sqlx::Result<Vec<RepPersonalBest>> {
    sqlx::query_as!(
        RepPersonalBest,
        r#"
        WITH rep_pbs AS (
            SELECT MAX(weight) AS weight, reps
            FROM freeform_exercise fe
            JOIN workout w ON fe.workout_id = w.id
            JOIN account a ON w.account_id = a.id
            WHERE a.account_uid = $1
            AND fe.description = $2
            GROUP BY reps
        )
        SELECT DISTINCT ON (rp.weight, rp.reps)
            rp.weight AS "weight!",
            rp.reps,
            w.recorded AS "recorded: forms::RecordedDate"
        FROM rep_pbs rp
        JOIN freeform_exercise fe ON (rp.weight = fe.weight AND rp.reps = fe.reps)
        JOIN workout w ON fe.workout_id = w.id
        JOIN account a ON w.account_id = a.id
        WHERE a.account_uid = $1
        AND fe.description = $2
        ORDER BY reps
        "#,
        user_id,
        description
    )
    .fetch_all(conn.deref_mut())
    .await
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize)]
pub struct BodyweightStatistics {
    pub increase_last_week: Option<f32>,
    pub increase_last_month: Option<f32>,
    pub average_last_week: Option<f64>,
    pub average_last_month: Option<f64>,
}

pub async fn get_bodyweight_statistics(
    user_id: Uuid,
    conn: &mut Connection,
) -> sqlx::Result<BodyweightStatistics> {
    sqlx::query_as!(
        BodyweightStatistics,
        r#"
        WITH account_id AS (
            SELECT id
            FROM account
            WHERE account_uid = $1
        )
        SELECT
            bodyweight - (
                SELECT bodyweight
                FROM bodyweight
                WHERE bodyweight.account_id = account_id
                AND recorded > now() - INTERVAL '1 week'
                ORDER BY recorded ASC
                LIMIT 1
            )
            AS increase_last_week,
            bodyweight - (
                SELECT bodyweight
                FROM bodyweight
                WHERE bodyweight.account_id = account_id
                AND recorded > now() - INTERVAL '1 month'
                ORDER BY recorded ASC
                LIMIT 1
            )
            AS increase_last_month,
            (
                SELECT AVG(bodyweight)
                FROM bodyweight
                WHERE bodyweight.account_id = account_id
                AND recorded > now() - INTERVAL '1 week'
            ) AS average_last_week,
            (
                SELECT AVG(bodyweight)
                FROM bodyweight
                WHERE bodyweight.account_id = account_id
                AND recorded > now() - INTERVAL '1 month'
            ) AS average_last_month
        FROM bodyweight
        WHERE bodyweight.account_id = account_id
        ORDER BY recorded DESC
        LIMIT 1
        "#,
        user_id
    )
    .fetch_one(conn.deref_mut())
    .await
}

#[derive(Copy, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkoutStatistics {
    pub squat_volume_past_week: Option<f64>,
    pub bench_volume_past_week: Option<f64>,
    pub deadlift_volume_past_week: Option<f64>,
    pub other_volume_past_week: Option<f64>,
}

pub async fn get_workout_statistics(
    user_id: Uuid,
    end: chrono::NaiveDate,
    conn: &mut Connection,
) -> sqlx::Result<WorkoutStatistics> {
    let one_week_prior = end - Duration::weeks(1);

    sqlx::query_as!(
        WorkoutStatistics,
        r#"
        SELECT
        (
            SELECT sum(se.weight * se.reps * se.sets)
            FROM structured_exercise se
            JOIN workout w ON w.id = se.workout_id
            JOIN account a ON a.id = w.account_id
            WHERE a.account_uid = $1
            AND w.recorded BETWEEN $2 AND $3
            AND se.variant = 'Squat'
        )
        AS squat_volume_past_week,
        (
            SELECT sum(se.weight * se.reps * se.sets)
            FROM structured_exercise se
            JOIN workout w ON w.id = se.workout_id
            JOIN account a ON a.id = w.account_id
            WHERE a.account_uid = $1
            AND w.recorded BETWEEN $2 AND $3
            AND se.variant = 'Bench'
        )
        AS bench_volume_past_week,
        (
            SELECT sum(se.weight * se.reps * se.sets)
            FROM structured_exercise se
            JOIN workout w ON w.id = se.workout_id
            JOIN account a ON a.id = w.account_id
            WHERE a.account_uid = $1
            AND w.recorded BETWEEN $2 AND $3
            AND se.variant = 'Deadlift'
        )
        AS deadlift_volume_past_week,
        (
            SELECT sum(fe.weight * fe.reps * fe.sets)
            FROM freeform_exercise fe
            JOIN workout w ON w.id = fe.workout_id
            JOIN account a ON a.id = w.account_id
            WHERE a.account_uid = $1
            AND w.recorded BETWEEN $2 AND $3
        )
        AS other_volume_past_week
        "#,
        user_id,
        one_week_prior,
        end,
    )
    .fetch_one(conn.deref_mut())
    .await
}
