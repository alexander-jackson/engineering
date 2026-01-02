use std::ops::DerefMut;

use uuid::Uuid;

use crate::forms::{Exercise, ExerciseVariant};
use crate::persistence::Connection;

pub async fn insert(
    workout_id: Uuid,
    exercise: &Exercise,
    conn: &mut Connection,
) -> sqlx::Result<()> {
    match exercise.variant {
        ExerciseVariant::Other => insert_other_exercise(workout_id, exercise, conn).await,
        _ => insert_structured_exercise(workout_id, exercise, conn).await,
    }
}

async fn insert_structured_exercise(
    workout_id: Uuid,
    exercise: &Exercise,
    conn: &mut Connection,
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
    .execute(conn.deref_mut())
    .await?;

    Ok(())
}

async fn insert_other_exercise(
    workout_id: Uuid,
    exercise: &Exercise,
    conn: &mut Connection,
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
    .execute(conn.deref_mut())
    .await?;

    Ok(())
}

pub async fn fetch_unique(
    user_id: Uuid,
    variant: ExerciseVariant,
    conn: &mut Connection,
) -> sqlx::Result<Vec<String>> {
    match variant {
        ExerciseVariant::Other => fetch_freeform_unique(user_id, conn).await,
        _ => fetch_structured_unique(user_id, variant, conn).await,
    }
}

pub async fn fetch_structured_unique(
    user_id: Uuid,
    variant: ExerciseVariant,
    conn: &mut Connection,
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
    .fetch_all(conn.deref_mut())
    .await
}

pub async fn fetch_freeform_unique(
    user_id: Uuid,
    conn: &mut Connection,
) -> sqlx::Result<Vec<String>> {
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
    .fetch_all(conn.deref_mut())
    .await
}

pub async fn fetch_for_workout(
    workout_id: Uuid,
    conn: &mut Connection,
) -> sqlx::Result<Vec<Exercise>> {
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
    .fetch_all(conn.deref_mut())
    .await
}

pub async fn delete_by_date(
    user_id: Uuid,
    recorded: chrono::NaiveDate,
    conn: &mut Connection,
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
    .execute(conn.deref_mut())
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
    .execute(conn.deref_mut())
    .await?;

    Ok(())
}

pub async fn rename(
    user_id: Uuid,
    variant: ExerciseVariant,
    description: &str,
    updated: &str,
    conn: &mut Connection,
) -> sqlx::Result<u64> {
    match variant {
        ExerciseVariant::Other => rename_freeform(user_id, description, updated, conn).await,
        _ => rename_structured(user_id, variant, description, updated, conn).await,
    }
}

pub async fn rename_freeform(
    user_id: Uuid,
    description: &str,
    updated: &str,
    conn: &mut Connection,
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
    .execute(conn.deref_mut())
    .await
    .map(|r| r.rows_affected())
}

pub async fn rename_structured(
    user_id: Uuid,
    variant: ExerciseVariant,
    description: &str,
    updated: &str,
    conn: &mut Connection,
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
    .execute(conn.deref_mut())
    .await
    .map(|r| r.rows_affected())
}
