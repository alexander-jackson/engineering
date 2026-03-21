#![allow(dead_code)]

use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{PgPool, Result};

use crate::uid::{CertificateUid, DomainUid};

pub async fn insert_domain(pool: &PgPool, domain: &str) -> Result<DomainUid> {
    let domain_uid = DomainUid::new();
    let created_at = Utc::now();

    sqlx::query!(
        "INSERT INTO domain (domain_uid, name, created_at) VALUES ($1, $2, $3)",
        *domain_uid,
        domain,
        created_at
    )
    .execute(pool)
    .await?;

    Ok(domain_uid)
}

pub async fn insert_certificate(
    pool: &PgPool,
    domain_uid: DomainUid,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
) -> Result<()> {
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
    .execute(pool)
    .await?;

    Ok(())
}

pub struct DomainCertificateInfo {
    pub domain: String,
    pub expires_at: DateTime<Utc>,
}

pub async fn select_latest_expiry_per_domain(pool: &PgPool) -> Result<Vec<DomainCertificateInfo>> {
    let rows = sqlx::query_as!(
        DomainCertificateInfo,
        r#"
            SELECT d.name AS domain, c.expires_at
            FROM domain d
            JOIN LATERAL (
                SELECT expires_at
                FROM certificate
                WHERE domain_id = d.id
                ORDER BY expires_at DESC
                LIMIT 1
            ) c ON true
            ORDER BY c.expires_at ASC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

#[cfg(test)]
mod tests;
