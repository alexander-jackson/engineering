use std::ops::DerefMut;

use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{PgPool, Result};

use crate::uid::{CertificateUid, DomainStatusChangeUid, DomainUid};

#[derive(Clone, Copy, Debug, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum DomainStatus {
    Active,
    Retired,
}

impl DomainStatus {
    /// The name of this status in the `domain_status` table.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Retired => "Retired",
        }
    }
}

pub type Transaction<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

pub async fn insert_domain(tx: &mut Transaction<'_>, domain: &str) -> Result<DomainUid> {
    let domain_uid = DomainUid::new();
    let created_at = Utc::now();

    sqlx::query!(
        "INSERT INTO domain (domain_uid, name, created_at) VALUES ($1, $2, $3)",
        *domain_uid,
        domain,
        created_at
    )
    .execute(tx.deref_mut())
    .await?;

    insert_domain_status_change(tx, domain_uid, DomainStatus::Active).await?;

    Ok(domain_uid)
}

pub async fn insert_certificate(
    tx: &mut Transaction<'_>,
    domain_uid: DomainUid,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
) -> Result<CertificateUid> {
    let certificate_uid = CertificateUid::new();

    sqlx::query!(
        r#"
            INSERT INTO certificate (certificate_uid, domain_id, created_at, expires_at)
            VALUES (
                $1,
                (SELECT id FROM domain WHERE domain_uid = $2 LIMIT 1),
                $3,
                $4
            )
        "#,
        *certificate_uid,
        *domain_uid,
        created_at,
        expires_at
    )
    .execute(tx.deref_mut())
    .await?;

    Ok(certificate_uid)
}

#[derive(Clone, Debug)]
pub struct DomainCertificateInfo {
    pub domain_uid: DomainUid,
    pub domain: String,
    pub expires_at: DateTime<Utc>,
}

pub async fn select_latest_expiry_per_domain(pool: &PgPool) -> Result<Vec<DomainCertificateInfo>> {
    let rows = sqlx::query_as!(
        DomainCertificateInfo,
        r#"
            SELECT d.domain_uid, d.name AS domain, c.expires_at
            FROM domain d
            JOIN LATERAL (
                SELECT expires_at
                FROM certificate
                WHERE domain_id = d.id
                ORDER BY expires_at DESC
                LIMIT 1
            ) c ON true
            WHERE (
                SELECT ds.name
                FROM domain_status_change dsc
                JOIN domain_status ds ON ds.id = dsc.domain_status_id
                WHERE dsc.domain_id = d.id
                ORDER BY dsc.created_at DESC, dsc.id DESC
                LIMIT 1
            ) = $1
            ORDER BY c.expires_at ASC
        "#,
        DomainStatus::Active.as_str()
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

#[derive(Clone, Debug)]
pub struct DomainRecord {
    pub domain_uid: DomainUid,
    pub name: String,
    pub status: DomainStatus,
}

pub async fn select_domain_by_name(
    tx: &mut Transaction<'_>,
    name: &str,
) -> Result<Option<DomainRecord>> {
    sqlx::query_as!(
        DomainRecord,
        r#"
            SELECT
                d.domain_uid,
                d.name,
                (
                    SELECT ds.name
                    FROM domain_status_change dsc
                    JOIN domain_status ds ON ds.id = dsc.domain_status_id
                    WHERE dsc.domain_id = d.id
                    ORDER BY dsc.created_at DESC, dsc.id DESC
                    LIMIT 1
                ) AS "status!: DomainStatus"
            FROM domain d
            WHERE d.name = $1
        "#,
        name
    )
    .fetch_optional(tx.deref_mut())
    .await
}

pub async fn select_domain_by_uid(
    tx: &mut Transaction<'_>,
    domain_uid: DomainUid,
) -> Result<Option<DomainRecord>> {
    sqlx::query_as!(
        DomainRecord,
        r#"
            SELECT
                d.domain_uid,
                d.name,
                (
                    SELECT ds.name
                    FROM domain_status_change dsc
                    JOIN domain_status ds ON ds.id = dsc.domain_status_id
                    WHERE dsc.domain_id = d.id
                    ORDER BY dsc.created_at DESC, dsc.id DESC
                    LIMIT 1
                ) AS "status!: DomainStatus"
            FROM domain d
            WHERE d.domain_uid = $1
        "#,
        *domain_uid
    )
    .fetch_optional(tx.deref_mut())
    .await
}

pub async fn is_domain_active(pool: &PgPool, domain_uid: DomainUid) -> Result<bool> {
    let status = sqlx::query_scalar!(
        r#"
            SELECT (
                SELECT ds.name
                FROM domain_status_change dsc
                JOIN domain_status ds ON ds.id = dsc.domain_status_id
                WHERE dsc.domain_id = d.id
                ORDER BY dsc.created_at DESC, dsc.id DESC
                LIMIT 1
            ) AS "status!: DomainStatus"
            FROM domain d
            WHERE d.domain_uid = $1
        "#,
        *domain_uid
    )
    .fetch_optional(pool)
    .await?;

    Ok(status == Some(DomainStatus::Active))
}

pub async fn retire_domain(tx: &mut Transaction<'_>, domain_uid: DomainUid) -> Result<()> {
    insert_domain_status_change(tx, domain_uid, DomainStatus::Retired).await
}

pub async fn resurrect_domain(tx: &mut Transaction<'_>, domain_uid: DomainUid) -> Result<()> {
    insert_domain_status_change(tx, domain_uid, DomainStatus::Active).await
}

async fn insert_domain_status_change(
    tx: &mut Transaction<'_>,
    domain_uid: DomainUid,
    status: DomainStatus,
) -> Result<()> {
    let change_uid = DomainStatusChangeUid::new();
    let created_at = Utc::now();

    sqlx::query!(
        r#"
            INSERT INTO domain_status_change (
                domain_status_change_uid, domain_id, domain_status_id, created_at
            )
            VALUES (
                $1,
                (SELECT id FROM domain WHERE domain_uid = $2),
                (SELECT id FROM domain_status WHERE name = $3),
                $4
            )
        "#,
        *change_uid,
        *domain_uid,
        status.as_str(),
        created_at
    )
    .execute(tx.deref_mut())
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests;
