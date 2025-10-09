use std::ops::DerefMut;

use chrono::Duration;
use color_eyre::eyre::Result;
use serde::Serialize;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres};
use sqlx_bootstrap::{ApplicationConfig, BootstrapConfig, ConnectionConfig, RootConfig};
use uuid::Uuid;

use crate::poller::FailureReason;
use crate::utils::get_env_var;

type Transaction = sqlx::Transaction<'static, Postgres>;

pub async fn bootstrap() -> Result<PgPool> {
    let root_username = get_env_var("ROOT_USERNAME")?;
    let root_password = get_env_var("ROOT_PASSWORD")?;
    let root_database = get_env_var("ROOT_DATABASE")?;

    let app_username = get_env_var("APP_USERNAME")?;
    let app_password = get_env_var("APP_PASSWORD")?;
    let app_database = get_env_var("APP_DATABASE")?;

    let host = get_env_var("DATABASE_HOST")?;
    let port = get_env_var("DATABASE_PORT")?.parse()?;

    let root_config = RootConfig::new(&root_username, &root_password, &root_database);
    let app_config = ApplicationConfig::new(&app_username, &app_password, &app_database);
    let conn_config = ConnectionConfig::new(&host, port);

    let config = BootstrapConfig::new(root_config, app_config, conn_config);
    let pool = config.bootstrap().await?;

    sqlx::migrate!().run(&pool).await?;

    Ok(pool)
}

#[derive(Serialize)]
pub struct Origin {
    pub origin_uid: Uuid,
    pub uri: String,
}

pub async fn insert_origin(pool: &PgPool, origin_uid: Uuid, uri: &str) -> Result<()> {
    sqlx::query!(
        r#"
            INSERT INTO origin (origin_uid, uri)
            VALUES ($1, $2)
        "#,
        origin_uid,
        uri,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn fetch_origins(pool: &PgPool) -> Result<Vec<Origin>> {
    let origins = sqlx::query_as!(
        Origin,
        r#"
            SELECT origin_uid, uri
            FROM origin
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(origins)
}

pub struct IndexOrigin {
    pub origin_uid: Uuid,
    pub uri: String,
    pub status: i16,
    pub latency_millis: i64,
    pub queried_at: DateTime<Utc>,
}

pub async fn fetch_origins_with_most_recent_success_metrics(
    pool: &PgPool,
) -> Result<Vec<IndexOrigin>> {
    let origins = sqlx::query_as!(
        IndexOrigin,
        r#"
            SELECT DISTINCT ON (o.uri)
                o.origin_uid,
                o.uri,
                q.status,
                q.latency_millis,
                q.queried_at
            FROM origin o
            JOIN query q ON o.id = q.origin_id
            ORDER BY o.uri, q.queried_at DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(origins)
}

pub struct OriginFailure {
    pub origin_uid: Uuid,
    pub uri: String,
    pub failure_reason: String,
    pub queried_at: DateTime<Utc>,
}

pub async fn fetch_origins_with_most_recent_failure_metrics(
    pool: &PgPool,
) -> Result<Vec<OriginFailure>> {
    let origins = sqlx::query_as!(
        OriginFailure,
        r#"
            SELECT DISTINCT ON (o.uri)
                o.origin_uid,
                o.uri,
                qfr.name AS failure_reason,
                qf.queried_at
            FROM origin o
            JOIN query_failure qf ON o.id = qf.origin_id
            JOIN query_failure_reason qfr ON qfr.id = qf.failure_reason_id
            ORDER BY o.uri, qf.queried_at DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(origins)
}

pub async fn insert_query(
    tx: &mut Transaction,
    origin_uid: Uuid,
    status: u16,
    latency_millis: i64,
    queried_at: DateTime<Utc>,
) -> Result<Uuid> {
    let query_uid = Uuid::new_v4();

    sqlx::query!(
        r#"
            INSERT INTO query (query_uid, origin_id, status, latency_millis, queried_at)
            VALUES (
                $1,
                (SELECT id FROM origin WHERE origin_uid = $2),
                $3,
                $4,
                $5
            )
        "#,
        query_uid,
        origin_uid,
        status as i16,
        latency_millis,
        queried_at
    )
    .execute(tx.deref_mut())
    .await?;

    Ok(query_uid)
}

pub async fn insert_query_failure(
    tx: &mut Transaction,
    origin_uid: Uuid,
    failure_reason: FailureReason,
    queried_at: DateTime<Utc>,
) -> Result<Uuid> {
    let query_failure_uid = Uuid::new_v4();

    sqlx::query!(
        r#"
            INSERT INTO query_failure (query_failure_uid, origin_id, failure_reason_id, queried_at)
            VALUES (
                $1,
                (SELECT id FROM origin WHERE origin_uid = $2),
                (SELECT id FROM query_failure_reason WHERE name = $3),
                $4
            )
        "#,
        query_failure_uid,
        origin_uid,
        failure_reason.as_str(),
        queried_at
    )
    .execute(tx.deref_mut())
    .await?;

    Ok(query_failure_uid)
}

pub async fn failure_rate_exceeded(
    pool: &PgPool,
    origin_uid: Uuid,
    limit: u16,
    period: Duration,
) -> Result<bool> {
    let end = Utc::now();
    let start = end - period;

    let exceeded = sqlx::query_scalar!(
        r#"
            SELECT COUNT(*) >= $2
            FROM query_failure qf
            JOIN origin o ON o.id = qf.origin_id
            WHERE o.origin_uid = $1
            AND qf.queried_at BETWEEN $3 AND $4
        "#,
        origin_uid,
        limit as i32,
        start,
        end,
    )
    .fetch_one(pool)
    .await?
    .expect("Count returned a null value");

    Ok(exceeded)
}

pub enum NotificationType {
    Uptime,
    CertificateExpiry,
}

impl NotificationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Uptime => "Uptime",
            Self::CertificateExpiry => "CertificateExpiry",
        }
    }
}

pub async fn insert_notification(
    pool: &PgPool,
    origin_uid: Uuid,
    notification_type: NotificationType,
    topic: &str,
    subject: &str,
    message: &str,
    created_at: DateTime<Utc>,
) -> Result<Uuid> {
    let notification_uid = Uuid::new_v4();

    sqlx::query!(
        r#"
            INSERT INTO notification (notification_uid, origin_id, notification_type_id, topic, subject, message, created_at)
            VALUES (
                $1,
                (SELECT id FROM origin WHERE origin_uid = $2),
                (SELECT id FROM notification_type WHERE name = $3),
                $4,
                $5,
                $6,
                $7
            )
        "#,
        notification_uid,
        origin_uid,
        notification_type.as_str(),
        topic,
        subject,
        message,
        created_at
    )
    .execute(pool)
    .await?;

    Ok(notification_uid)
}

pub async fn latest_notification_older_than(
    pool: &PgPool,
    origin_uid: Uuid,
    notification_type: NotificationType,
    cooldown: Duration,
) -> Result<bool> {
    let boundary = Utc::now() - cooldown;

    let notification = sqlx::query_scalar!(
        r#"
            SELECT NOT EXISTS (
                SELECT
                FROM notification n
                JOIN origin o ON o.id = n.origin_id
                JOIN notification_type nt ON nt.id = n.notification_type_id
                WHERE o.origin_uid = $1
                AND nt.name = $2
                AND n.created_at > $3
                LIMIT 1
            )
        "#,
        origin_uid,
        notification_type.as_str(),
        boundary,
    )
    .fetch_one(pool)
    .await?
    .expect("Exists returned a null value");

    Ok(notification)
}

pub struct OriginDetail {
    pub origin_uid: Uuid,
    pub uri: String,
}

pub async fn fetch_origin_by_uid(pool: &PgPool, origin_uid: Uuid) -> Result<Option<OriginDetail>> {
    let origin = sqlx::query_as!(
        OriginDetail,
        r#"
            SELECT origin_uid, uri
            FROM origin
            WHERE origin_uid = $1
        "#,
        origin_uid
    )
    .fetch_optional(pool)
    .await?;

    Ok(origin)
}

pub struct RecentQuery {
    pub is_success: bool,
    pub status: Option<i16>,
    pub latency_millis: Option<i64>,
    pub failure_reason: Option<String>,
    pub queried_at: DateTime<Utc>,
}

pub async fn fetch_recent_queries(
    pool: &PgPool,
    origin_uid: Uuid,
    limit: i64,
) -> Result<Vec<RecentQuery>> {
    let queries = sqlx::query_as!(
        RecentQuery,
        r#"
            SELECT
                combined.is_success AS "is_success!",
                combined.status AS "status?",
                combined.latency_millis AS "latency_millis?",
                combined.failure_reason AS "failure_reason?",
                combined.queried_at AS "queried_at!"
            FROM (
                SELECT
                    TRUE AS is_success,
                    q.status,
                    q.latency_millis,
                    NULL::TEXT AS failure_reason,
                    q.queried_at
                FROM query q
                JOIN origin o ON o.id = q.origin_id
                WHERE o.origin_uid = $1

                UNION ALL

                SELECT
                    FALSE AS is_success,
                    NULL::SMALLINT AS status,
                    NULL::BIGINT AS latency_millis,
                    qfr.name AS failure_reason,
                    qf.queried_at
                FROM query_failure qf
                JOIN origin o ON o.id = qf.origin_id
                JOIN query_failure_reason qfr ON qfr.id = qf.failure_reason_id
                WHERE o.origin_uid = $1
            ) combined
            ORDER BY combined.queried_at DESC
            LIMIT $2
        "#,
        origin_uid,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(queries)
}

pub async fn fetch_most_recent_success(
    pool: &PgPool,
    origin_uid: Uuid,
) -> Result<Option<DateTime<Utc>>> {
    let result = sqlx::query_scalar!(
        r#"
            SELECT q.queried_at
            FROM query q
            JOIN origin o ON o.id = q.origin_id
            WHERE o.origin_uid = $1
            ORDER BY q.queried_at DESC
            LIMIT 1
        "#,
        origin_uid
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

pub async fn fetch_most_recent_failure(
    pool: &PgPool,
    origin_uid: Uuid,
) -> Result<Option<DateTime<Utc>>> {
    let result = sqlx::query_scalar!(
        r#"
            SELECT qf.queried_at
            FROM query_failure qf
            JOIN origin o ON o.id = qf.origin_id
            WHERE o.origin_uid = $1
            ORDER BY qf.queried_at DESC
            LIMIT 1
        "#,
        origin_uid
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

pub async fn insert_certificate_check(
    pool: &PgPool,
    origin_uid: Uuid,
    expires_at: DateTime<Utc>,
    checked_at: DateTime<Utc>,
) -> Result<Uuid> {
    let certificate_check_uid = Uuid::new_v4();

    sqlx::query!(
        r#"
            INSERT INTO certificate_check (certificate_check_uid, origin_id, expires_at, checked_at)
            VALUES (
                $1,
                (SELECT id FROM origin WHERE origin_uid = $2),
                $3,
                $4
            )
        "#,
        certificate_check_uid,
        origin_uid,
        expires_at,
        checked_at
    )
    .execute(pool)
    .await?;

    Ok(certificate_check_uid)
}

pub struct CertificateCheck {
    pub expires_at: DateTime<Utc>,
    pub checked_at: DateTime<Utc>,
}

pub async fn fetch_most_recent_certificate_check(
    pool: &PgPool,
    origin_uid: Uuid,
) -> Result<Option<CertificateCheck>> {
    let result = sqlx::query_as!(
        CertificateCheck,
        r#"
            SELECT cc.expires_at, cc.checked_at
            FROM certificate_check cc
            JOIN origin o ON o.id = cc.origin_id
            WHERE o.origin_uid = $1
            ORDER BY cc.checked_at DESC
            LIMIT 1
        "#,
        origin_uid
    )
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

pub async fn certificate_expires_within(
    pool: &PgPool,
    origin_uid: Uuid,
    threshold: Duration,
) -> Result<Option<CertificateCheck>> {
    let boundary = Utc::now() + threshold;

    let cert_check = sqlx::query_as!(
        CertificateCheck,
        r#"
            SELECT cc.expires_at, cc.checked_at
            FROM certificate_check cc
            JOIN origin o ON o.id = cc.origin_id
            WHERE o.origin_uid = $1
            AND cc.expires_at <= $2
            ORDER BY cc.checked_at DESC
            LIMIT 1
        "#,
        origin_uid,
        boundary,
    )
    .fetch_optional(pool)
    .await?;

    Ok(cert_check)
}

#[derive(Serialize)]
pub struct RecentNotification {
    pub notification_uid: Uuid,
    pub origin_uid: Uuid,
    pub uri: String,
    pub notification_type: String,
    pub subject: String,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

pub async fn fetch_recent_notifications(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<RecentNotification>> {
    let notifications = sqlx::query_as!(
        RecentNotification,
        r#"
            SELECT
                n.notification_uid,
                o.origin_uid,
                o.uri,
                nt.name AS notification_type,
                n.subject,
                n.message,
                n.created_at
            FROM notification n
            JOIN origin o ON o.id = n.origin_id
            JOIN notification_type nt ON nt.id = n.notification_type_id
            ORDER BY n.created_at DESC
            LIMIT $1
        "#,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(notifications)
}
