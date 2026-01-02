use std::ops::DerefMut;

use uuid::Uuid;

use crate::forms;
use crate::persistence::Connection;

#[derive(Debug, Serialize)]
pub struct BodyweightRecord {
    pub recorded: forms::RecordedDate,
    pub bodyweight: f32,
}

#[derive(Debug, Serialize)]
pub struct SpecificBodyweightRecord {
    pub bodyweight: f32,
}

pub async fn insert(
    user_id: Uuid,
    bodyweight: f32,
    recorded: chrono::NaiveDate,
    conn: &mut Connection,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO bodyweight (account_id, bodyweight, recorded)
        VALUES ((SELECT id FROM account WHERE account_uid = $1), $2, $3)
        ON CONFLICT (account_id, recorded) DO UPDATE
            SET bodyweight = EXCLUDED.bodyweight"#,
        user_id,
        bodyweight,
        recorded,
    )
    .execute(conn.deref_mut())
    .await?;

    Ok(())
}

pub async fn fetch_by_date(
    user_id: Uuid,
    recorded: chrono::NaiveDate,
    conn: &mut Connection,
) -> sqlx::Result<Option<SpecificBodyweightRecord>> {
    let contents = sqlx::query_as!(
        SpecificBodyweightRecord,
        r#"
        SELECT bodyweight
        FROM bodyweight b
        JOIN account a ON a.id = b.account_id
        WHERE a.account_uid = $1 AND b.recorded = $2
        "#,
        user_id,
        recorded,
    )
    .fetch_optional(conn.deref_mut())
    .await?;

    Ok(contents)
}

pub async fn fetch_all(
    user_id: Uuid,
    conn: &mut Connection,
) -> sqlx::Result<Vec<BodyweightRecord>> {
    sqlx::query_as!(
        BodyweightRecord,
        r#"
        SELECT
            b.recorded AS "recorded: forms::RecordedDate",
            bodyweight
        FROM bodyweight b
        JOIN account a ON a.id = b.account_id
        WHERE a.account_uid = $1
        ORDER BY b.recorded
        "#,
        user_id
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
        DELETE FROM bodyweight
        WHERE account_id = (
            SELECT id
            FROM account
            WHERE account_uid = $1
        )
        AND recorded = $2
        "#,
        user_id,
        recorded,
    )
    .execute(conn.deref_mut())
    .await?;

    Ok(())
}

pub async fn fetch_most_recent(
    user_id: Uuid,
    conn: &mut Connection,
) -> sqlx::Result<Option<BodyweightRecord>> {
    sqlx::query_as!(
        BodyweightRecord,
        r#"
        SELECT
            b.bodyweight,
            b.recorded AS "recorded: forms::RecordedDate"
        FROM bodyweight b
        JOIN account a ON a.id = b.account_id
        WHERE a.account_uid = $1
        ORDER BY recorded DESC
        LIMIT 1
        "#,
        user_id
    )
    .fetch_optional(conn.deref_mut())
    .await
}
