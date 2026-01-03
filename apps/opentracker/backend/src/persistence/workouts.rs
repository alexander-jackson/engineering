use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::forms::{self, ExerciseVariant};
use crate::persistence;

#[derive(Clone)]
pub struct DatedExercise {
    pub recorded: forms::RecordedDate,
    pub variant: forms::ExerciseVariant,
    pub description: String,
    pub weight: f32,
    pub reps: i32,
    pub sets: i32,
    pub rpe: Option<f32>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct DatedWorkout {
    pub recorded: forms::RecordedDate,
    pub exercises: Vec<forms::Exercise>,
}

pub async fn create_or_fetch(
    user_id: Uuid,
    recorded: chrono::NaiveDate,
    pool: &PgPool,
) -> sqlx::Result<Uuid> {
    let uuid = Uuid::new_v4();

    let workout_uid = sqlx::query!(
        r#"
        WITH row (workout_uid, account_id, recorded)
        AS (VALUES ($1 :: UUID, (SELECT id FROM account WHERE account_uid = $2), $3 :: DATE)),
        insert AS (
            INSERT INTO workout (workout_uid, account_id, recorded)
            SELECT * FROM row
            ON CONFLICT ON CONSTRAINT uk_workout_account_id_recorded DO NOTHING
            RETURNING workout_uid
        )
        SELECT workout_uid FROM insert
        UNION ALL
        SELECT w.workout_uid FROM row
        JOIN workout w USING (account_id, recorded)
        "#,
        uuid,
        user_id,
        recorded,
    )
    .fetch_one(pool)
    .await?
    .workout_uid
    .expect("Expression should always return a value");

    Ok(workout_uid)
}

pub async fn fetch_exercises_for_workout(
    account_uid: Uuid,
    recorded: chrono::NaiveDate,
    pool: &PgPool,
) -> sqlx::Result<Vec<forms::Exercise>> {
    sqlx::query_as!(
        forms::Exercise,
        r#"
        SELECT
            variant AS "variant!: ExerciseVariant",
            description AS "description!",
            weight AS "weight!",
            reps AS "reps!",
            sets AS "sets!",
            rpe
        FROM structured_exercise se
        JOIN workout w ON w.id = se.workout_id
        JOIN account a ON a.id = w.account_id
        WHERE a.account_uid = $1
        AND w.recorded = $2

        UNION ALL

        SELECT
            variant AS "variant!: ExerciseVariant",
            description AS "description!",
            weight AS "weight!",
            reps AS "reps!",
            sets AS "sets!",
            rpe
        FROM freeform_exercise fe
        JOIN workout w ON w.id = fe.workout_id
        JOIN account a ON a.id = w.account_id
        WHERE a.account_uid = $1
        AND w.recorded = $2
        "#,
        account_uid,
        recorded,
    )
    .fetch_all(pool)
    .await
}

pub async fn fetch_with_exercises_between(
    user_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    pool: &PgPool,
) -> sqlx::Result<Vec<DatedExercise>> {
    // Unfortunately, `sqlx` doesn't seem to like this query (or Postgres can't interpret the types
    // correctly). Thus, we must use `unchecked` here. In the future, I'll add a whole load of
    // tests for this to make sure it works as expected.
    sqlx::query_as_unchecked!(
        DatedExercise,
        r#"
        SElECT
            recorded,
            variant,
            description,
            weight,
            reps,
            sets,
            rpe
        FROM workout w
        INNER JOIN structured_exercise se ON se.workout_id = w.id
        WHERE account_id = (SELECT id FROM account WHERE account_uid = $1)
        AND recorded BETWEEN $2 AND $3

        UNION ALL

        SElECT
            recorded,
            variant,
            description,
            weight,
            reps,
            sets,
            rpe
        FROM workout w
        INNER JOIN freeform_exercise fe ON fe.workout_id = w.id
        WHERE account_id = (SELECT id FROM account WHERE account_uid = $1)
        AND recorded BETWEEN $2 AND $3

        ORDER BY recorded DESC, variant, description, weight DESC
        "#,
        user_id,
        start,
        end,
    )
    .fetch_all(pool)
    .await
}

pub async fn delete_by_date(
    user_id: Uuid,
    recorded: chrono::NaiveDate,
    pool: &PgPool,
) -> sqlx::Result<()> {
    // Delete the exercises contained
    persistence::exercises::delete_by_date(user_id, recorded, pool).await?;

    sqlx::query!(
        r#"
        DELETE FROM workout
        WHERE account_id = (SELECT id FROM account WHERE account_uid = $1)
        AND recorded = $2
        "#,
        user_id,
        recorded,
    )
    .execute(pool)
    .await?;

    Ok(())
}
