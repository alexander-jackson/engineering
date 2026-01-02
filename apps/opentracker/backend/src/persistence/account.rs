use std::ops::DerefMut;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::persistence::Connection;

pub struct Account {
    pub account_uid: Uuid,
    pub email_address: String,
    pub password: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailVerificationStatus {
    pub email_address_uid: Uuid,
    pub email_address: String,
    pub verified_at: Option<DateTime<Utc>>,
}

pub async fn insert(email: &str, hashed: &str, conn: &mut Connection) -> sqlx::Result<Uuid> {
    let account_uid = Uuid::new_v4();
    let email_address_uid = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query!(
        r#"
        WITH email_address_record AS (
            INSERT INTO email_address (email_address_uid, email_address, created_at, active)
            VALUES ($1, $2, $3, true)
            RETURNING id
        )
        INSERT INTO account (account_uid, email_address_id, password, created_at)
        VALUES ($4, (SELECT id FROM email_address_record), $5, $3)
        "#,
        email_address_uid,
        email,
        now,
        account_uid,
        hashed,
    )
    .execute(conn.deref_mut())
    .await?;

    Ok(account_uid)
}

pub async fn find_by_id(id: Uuid, conn: &mut Connection) -> sqlx::Result<Option<Account>> {
    sqlx::query_as!(
        Account,
        r#"
        SELECT a.account_uid, ea.email_address, a.password, a.created_at
        FROM account a
        JOIN email_address ea ON a.email_address_id = ea.id
        WHERE a.account_uid = $1
        AND ea.active = TRUE
        "#,
        id
    )
    .fetch_optional(conn.deref_mut())
    .await
}

pub async fn find_by_email(email: &str, conn: &mut Connection) -> sqlx::Result<Option<Account>> {
    sqlx::query_as!(
        Account,
        r#"
        SELECT a.account_uid, ea.email_address, a.password, a.created_at
        FROM account a
        JOIN email_address ea ON a.email_address_id = ea.id
        WHERE ea.email_address ILIKE $1
        AND ea.active = TRUE
        "#,
        email
    )
    .fetch_optional(conn.deref_mut())
    .await
}

pub async fn update_password(id: Uuid, hashed: &str, conn: &mut Connection) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        UPDATE account
        SET password = $1
        WHERE account_uid = $2
        "#,
        hashed,
        id,
    )
    .execute(conn.deref_mut())
    .await?;

    Ok(())
}

pub async fn fetch_email_verification_status(
    id: Uuid,
    conn: &mut Connection,
) -> sqlx::Result<EmailVerificationStatus> {
    sqlx::query_as!(
        EmailVerificationStatus,
        r#"
        SELECT ea.email_address_uid, ea.email_address, ea.verified_at
        FROM email_address ea
        JOIN account a ON ea.id = a.email_address_id
        WHERE ea.active IS TRUE
        AND a.account_uid = $1
        "#,
        id,
    )
    .fetch_one(conn.deref_mut())
    .await
}

pub async fn verify_email(email_address_uid: Uuid, conn: &mut Connection) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        UPDATE email_address
        SET verified_at = now()::timestamp
        WHERE email_address_uid = $1
        "#,
        email_address_uid,
    )
    .execute(conn.deref_mut())
    .await?;

    Ok(())
}
