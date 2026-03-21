use std::time::Duration;

use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{PgPool, Result};

use crate::persistence::{insert_certificate, insert_domain, select_latest_expiry_per_domain};

/// Asserts that two timestamps are equal by comparing their microsecond representations.
fn assert_timestamp_equality(expected: &DateTime<Utc>, actual: &DateTime<Utc>) {
    assert_eq!(expected.timestamp_micros(), actual.timestamp_micros());
}

#[sqlx::test]
async fn can_insert_domains(pool: PgPool) -> Result<()> {
    let domain_uid = insert_domain(&pool, "example.com").await?;

    assert!(!domain_uid.is_nil());

    Ok(())
}

#[sqlx::test]
async fn can_insert_certificates(pool: PgPool) -> Result<()> {
    let domain_uid = insert_domain(&pool, "example.com").await?;
    let created_at = Utc::now();
    let expires_at = created_at + Duration::from_hours(24 * 90);

    insert_certificate(&pool, domain_uid, created_at, expires_at).await?;

    Ok(())
}

#[sqlx::test]
async fn can_select_latest_expiry_per_domain(pool: PgPool) -> Result<()> {
    let domain_uid = insert_domain(&pool, "example.com").await?;
    let created_at = Utc::now();
    let expires_at = created_at + Duration::from_hours(24 * 90);

    insert_certificate(&pool, domain_uid, created_at, expires_at).await?;

    let certs = select_latest_expiry_per_domain(&pool).await?;

    assert_eq!(certs.len(), 1);
    assert_eq!(certs[0].domain, "example.com");
    assert_timestamp_equality(&certs[0].expires_at, &expires_at);

    Ok(())
}

#[sqlx::test]
async fn certificates_expiries_are_returned_with_nearest_to_expiry_first(
    pool: PgPool,
) -> Result<()> {
    let domain_uid = insert_domain(&pool, "example.com").await?;
    let created_at = Utc::now();

    let first_renewal = created_at + Duration::from_hours(24 * 60);
    let first_expiry = first_renewal + Duration::from_hours(24 * 90);

    let second_expiry = first_renewal + Duration::from_hours(24 * 90);

    insert_certificate(&pool, domain_uid, created_at, first_expiry).await?;

    insert_certificate(&pool, domain_uid, first_renewal, second_expiry).await?;

    let certs = select_latest_expiry_per_domain(&pool).await?;

    assert_eq!(certs.len(), 1);
    assert_eq!(certs[0].domain, "example.com");
    assert_timestamp_equality(&certs[0].expires_at, &second_expiry);

    Ok(())
}

#[sqlx::test]
async fn can_handle_expiries_for_multiple_domains_and_sort_by_expiry(pool: PgPool) -> Result<()> {
    let domain_uid1 = insert_domain(&pool, "example.com").await?;
    let domain_uid2 = insert_domain(&pool, "example.org").await?;

    let created_at = Utc::now();

    let expiry1 = created_at + Duration::from_hours(24 * 90);
    let expiry2 = created_at + Duration::from_hours(24 * 60);

    insert_certificate(&pool, domain_uid1, created_at, expiry1).await?;
    insert_certificate(&pool, domain_uid2, created_at, expiry2).await?;

    let certs = select_latest_expiry_per_domain(&pool).await?;

    assert_eq!(certs.len(), 2);

    assert_eq!(certs[0].domain, "example.org");
    assert_timestamp_equality(&certs[0].expires_at, &expiry2);

    assert_eq!(certs[1].domain, "example.com");
    assert_timestamp_equality(&certs[1].expires_at, &expiry1);

    Ok(())
}
