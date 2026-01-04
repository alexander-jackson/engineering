use sqlx::PgPool;
use uuid::Uuid;

use crate::forms::{Exercise, ExerciseVariant};
use crate::persistence::workouts::DatedExercise;

pub async fn insert(workout_id: Uuid, exercise: &Exercise, pool: &PgPool) -> sqlx::Result<()> {
    match exercise.variant {
        ExerciseVariant::Other => insert_other_exercise(workout_id, exercise, pool).await,
        _ => insert_structured_exercise(workout_id, exercise, pool).await,
    }
}

async fn insert_structured_exercise(
    workout_id: Uuid,
    exercise: &Exercise,
    pool: &PgPool,
) -> sqlx::Result<()> {
    tracing::debug!(?exercise, "Inserting a structured exercise");

    sqlx::query!(
        r#"
        INSERT INTO structured_exercise (workout_id, variant, description, weight, reps, sets, rpe)
        VALUES ((SELECT id FROM workout WHERE workout_uid = $1), $2, $3, $4, $5, $6, $7)
        "#,
        workout_id,
        exercise.variant as ExerciseVariant,
        exercise.description,
        exercise.weight,
        exercise.reps,
        exercise.sets,
        exercise.rpe,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn insert_other_exercise(
    workout_id: Uuid,
    exercise: &Exercise,
    pool: &PgPool,
) -> sqlx::Result<()> {
    tracing::debug!(?exercise, "Inserting a freeform exercise");

    sqlx::query!(
        r#"
        INSERT INTO freeform_exercise (workout_id, variant, description, weight, reps, sets, rpe)
        VALUES ((SELECT id FROM workout WHERE workout_uid = $1), $2, $3, $4, $5, $6, $7)
        "#,
        workout_id,
        exercise.variant as ExerciseVariant,
        exercise.description,
        exercise.weight,
        exercise.reps,
        exercise.sets,
        exercise.rpe,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn fetch_unique(
    user_id: Uuid,
    variant: ExerciseVariant,
    pool: &PgPool,
) -> sqlx::Result<Vec<String>> {
    match variant {
        ExerciseVariant::Other => fetch_freeform_unique(user_id, pool).await,
        _ => fetch_structured_unique(user_id, variant, pool).await,
    }
}

pub async fn fetch_structured_unique(
    user_id: Uuid,
    variant: ExerciseVariant,
    pool: &PgPool,
) -> sqlx::Result<Vec<String>> {
    sqlx::query!(
        r#"
        SELECT DISTINCT description
        FROM structured_exercise
        WHERE variant = $2
        AND workout_id IN (
            SELECT id
            FROM workout
            WHERE account_id = (SELECT id FROM account WHERE account_uid = $1)
        )
        ORDER BY description
        "#,
        user_id,
        variant as ExerciseVariant,
    )
    .map(|e| e.description)
    .fetch_all(pool)
    .await
}

pub async fn fetch_freeform_unique(user_id: Uuid, pool: &PgPool) -> sqlx::Result<Vec<String>> {
    sqlx::query!(
        r#"
        SELECT DISTINCT description
        FROM freeform_exercise
        WHERE workout_id IN (
            SELECT id
            FROM workout
            WHERE account_id = (SELECT id FROM account WHERE account_uid = $1)
        )
        ORDER BY description
        "#,
        user_id
    )
    .map(|e| e.description)
    .fetch_all(pool)
    .await
}

pub async fn fetch_for_workout(workout_id: Uuid, pool: &PgPool) -> sqlx::Result<Vec<Exercise>> {
    sqlx::query_as!(
        Exercise,
        r#"
        WITH workout_id AS (
            SELECT id
            FROM workout
            WHERE workout_uid = $1
        )
        SELECT
            variant AS "variant!: ExerciseVariant",
            description AS "description!",
            weight AS "weight!",
            reps AS "reps!",
            sets AS "sets!",
            rpe
        FROM structured_exercise se
        WHERE se.workout_id = workout_id

        UNION ALL

        SELECT
            variant AS "variant!: ExerciseVariant",
            description AS "description!",
            weight AS "weight!",
            reps AS "reps!",
            sets AS "sets!",
            rpe
        FROM freeform_exercise fe
        WHERE fe.workout_id = workout_id
        "#,
        workout_id,
    )
    .fetch_all(pool)
    .await
}

pub async fn delete_by_date(
    user_id: Uuid,
    recorded: chrono::NaiveDate,
    pool: &PgPool,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        DELETE FROM structured_exercise
        WHERE workout_id = (
            SELECT id
            FROM workout
            WHERE account_id = (SELECT id FROM account WHERE account_uid = $1)
            AND recorded = $2
        )
        "#,
        user_id,
        recorded,
    )
    .execute(pool)
    .await?;

    sqlx::query!(
        r#"
        DELETE FROM freeform_exercise
        WHERE workout_id = (
            SELECT id
            FROM workout
            WHERE account_id = (SELECT id FROM account WHERE account_uid = $1)
            AND recorded = $2
        )
        "#,
        user_id,
        recorded,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn rename(
    user_id: Uuid,
    variant: ExerciseVariant,
    description: &str,
    updated: &str,
    pool: &PgPool,
) -> sqlx::Result<u64> {
    match variant {
        ExerciseVariant::Other => rename_freeform(user_id, description, updated, pool).await,
        _ => rename_structured(user_id, variant, description, updated, pool).await,
    }
}

pub async fn rename_freeform(
    user_id: Uuid,
    description: &str,
    updated: &str,
    pool: &PgPool,
) -> sqlx::Result<u64> {
    sqlx::query!(
        r#"
        UPDATE freeform_exercise
        SET description = $3
        WHERE description = $2
        AND workout_id IN (
            SELECT id
            FROM workout
            WHERE account_id = (SELECT id FROM account WHERE account_uid = $1)
        )
        "#,
        user_id,
        description,
        updated,
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
}

pub async fn rename_structured(
    user_id: Uuid,
    variant: ExerciseVariant,
    description: &str,
    updated: &str,
    pool: &PgPool,
) -> sqlx::Result<u64> {
    sqlx::query!(
        r#"
        UPDATE structured_exercise
        SET description = $4
        WHERE description = $3
        AND variant = $2
        AND workout_id IN (
            SELECT id
            FROM workout
            WHERE account_id = (SELECT id FROM account WHERE account_uid = $1)
        )
        "#,
        user_id,
        variant as ExerciseVariant,
        description,
        updated,
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
}

pub async fn fetch_last_session(
    user_id: Uuid,
    variant: ExerciseVariant,
    description: &str,
    exclude_date: chrono::NaiveDate,
    pool: &PgPool,
) -> sqlx::Result<Option<(Exercise, chrono::NaiveDate)>> {
    match variant {
        ExerciseVariant::Other => {
            fetch_last_session_freeform(user_id, description, exclude_date, pool).await
        }
        _ => fetch_last_session_structured(user_id, variant, description, exclude_date, pool).await,
    }
}

async fn fetch_last_session_structured(
    user_id: Uuid,
    variant: ExerciseVariant,
    description: &str,
    exclude_date: chrono::NaiveDate,
    pool: &PgPool,
) -> sqlx::Result<Option<(Exercise, chrono::NaiveDate)>> {
    let result = sqlx::query_as!(
        DatedExercise,
        r#"
        SELECT
            w.recorded AS "recorded!: _",
            se.variant AS "variant!: ExerciseVariant",
            se.description AS "description!",
            se.weight AS "weight!",
            se.reps AS "reps!",
            se.sets AS "sets!",
            se.rpe
        FROM structured_exercise se
        JOIN workout w ON w.id = se.workout_id
        JOIN account a ON a.id = w.account_id
        WHERE a.account_uid = $1
        AND se.variant = $2
        AND se.description = $3
        AND w.recorded < $4
        ORDER BY w.recorded DESC
        LIMIT 1
        "#,
        user_id,
        variant as ExerciseVariant,
        description,
        exclude_date,
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| {
        (
            Exercise {
                variant: r.variant,
                description: r.description,
                weight: r.weight,
                reps: r.reps,
                sets: r.sets,
                rpe: r.rpe,
            },
            r.recorded.0,
        )
    }))
}

async fn fetch_last_session_freeform(
    user_id: Uuid,
    description: &str,
    exclude_date: chrono::NaiveDate,
    pool: &PgPool,
) -> sqlx::Result<Option<(Exercise, chrono::NaiveDate)>> {
    let result = sqlx::query_as!(
        DatedExercise,
        r#"
        SELECT
            w.recorded AS "recorded!: _",
            fe.variant AS "variant!: ExerciseVariant",
            fe.description AS "description!",
            fe.weight AS "weight!",
            fe.reps AS "reps!",
            fe.sets AS "sets!",
            fe.rpe
        FROM freeform_exercise fe
        JOIN workout w ON w.id = fe.workout_id
        JOIN account a ON a.id = w.account_id
        WHERE a.account_uid = $1
        AND fe.description = $2
        AND w.recorded < $3
        ORDER BY w.recorded DESC
        LIMIT 1
        "#,
        user_id,
        description,
        exclude_date,
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| {
        (
            Exercise {
                variant: r.variant,
                description: r.description,
                weight: r.weight,
                reps: r.reps,
                sets: r.sets,
                rpe: r.rpe,
            },
            r.recorded.0,
        )
    }))
}
