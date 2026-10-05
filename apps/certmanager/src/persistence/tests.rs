use std::time::Duration;

use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{PgPool, Result};

use crate::persistence::{
    DomainStatus, insert_certificate, insert_domain, is_domain_active, resurrect_domain,
    retire_domain, select_domain_by_name, select_domain_by_uid, select_latest_expiry_per_domain,
};

/// Asserts that two timestamps are equal by comparing their microsecond representations.
fn assert_timestamp_equality(expected: &DateTime<Utc>, actual: &DateTime<Utc>) {
    assert_eq!(expected.timestamp_micros(), actual.timestamp_micros());
}

#[sqlx::test]
async fn can_insert_domains(pool: PgPool) -> Result<()> {
    let mut tx = pool.begin().await?;
    let domain_uid = insert_domain(&mut tx, "example.com").await?;

    assert!(!domain_uid.is_nil());

    Ok(())
}

#[sqlx::test]
async fn can_insert_certificates(pool: PgPool) -> Result<()> {
    let mut tx = pool.begin().await?;
    let domain_uid = insert_domain(&mut tx, "example.com").await?;
    let created_at = Utc::now();
    let expires_at = created_at + Duration::from_hours(24 * 90);

    insert_certificate(&mut tx, domain_uid, created_at, expires_at).await?;

    Ok(())
}

#[sqlx::test]
async fn can_select_latest_expiry_per_domain(pool: PgPool) -> Result<()> {
    let mut tx = pool.begin().await?;
    let domain_uid = insert_domain(&mut tx, "example.com").await?;
    let created_at = Utc::now();
    let expires_at = created_at + Duration::from_hours(24 * 90);

    insert_certificate(&mut tx, domain_uid, created_at, expires_at).await?;

    tx.commit().await?;

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
    let mut tx = pool.begin().await?;
    let domain_uid = insert_domain(&mut tx, "example.com").await?;
    let created_at = Utc::now();

    let first_renewal = created_at + Duration::from_hours(24 * 60);
    let first_expiry = first_renewal + Duration::from_hours(24 * 90);

    let second_expiry = first_renewal + Duration::from_hours(24 * 90);

    insert_certificate(&mut tx, domain_uid, created_at, first_expiry).await?;
    insert_certificate(&mut tx, domain_uid, first_renewal, second_expiry).await?;

    tx.commit().await?;

    let certs = select_latest_expiry_per_domain(&pool).await?;

    assert_eq!(certs.len(), 1);
    assert_eq!(certs[0].domain, "example.com");
    assert_timestamp_equality(&certs[0].expires_at, &second_expiry);

    Ok(())
}

#[sqlx::test]
async fn can_handle_expiries_for_multiple_domains_and_sort_by_name(pool: PgPool) -> Result<()> {
    let mut tx = pool.begin().await?;
    let domain_uid1 = insert_domain(&mut tx, "example.com").await?;
    let domain_uid2 = insert_domain(&mut tx, "example.org").await?;

    let created_at = Utc::now();

    let expiry1 = created_at + Duration::from_hours(24 * 90);
    let expiry2 = created_at + Duration::from_hours(24 * 60);

    insert_certificate(&mut tx, domain_uid1, created_at, expiry2).await?;
    insert_certificate(&mut tx, domain_uid2, created_at, expiry1).await?;

    tx.commit().await?;

    let certs = select_latest_expiry_per_domain(&pool).await?;

    assert_eq!(certs.len(), 2);

    assert_eq!(certs[0].domain, "example.com");
    assert_timestamp_equality(&certs[0].expires_at, &expiry2);

    assert_eq!(certs[1].domain, "example.org");
    assert_timestamp_equality(&certs[1].expires_at, &expiry1);

    Ok(())
}

#[sqlx::test]
async fn retired_domains_are_excluded_from_expiries(pool: PgPool) -> Result<()> {
    let mut tx = pool.begin().await?;
    let retired = insert_domain(&mut tx, "retired.com").await?;
    let active = insert_domain(&mut tx, "active.com").await?;

    let created_at = Utc::now();

    // The retired domain would sort first if it were still included
    insert_certificate(
        &mut tx,
        retired,
        created_at,
        created_at + Duration::from_hours(24),
    )
    .await?;
    insert_certificate(
        &mut tx,
        active,
        created_at,
        created_at + Duration::from_hours(48),
    )
    .await?;

    retire_domain(&mut tx, retired).await?;
    tx.commit().await?;

    let certs = select_latest_expiry_per_domain(&pool).await?;

    assert_eq!(certs.len(), 1);
    assert_eq!(certs[0].domain, "active.com");

    assert!(!is_domain_active(&pool, retired).await?);
    assert!(is_domain_active(&pool, active).await?);

    Ok(())
}

#[sqlx::test]
async fn latest_status_change_wins(pool: PgPool) -> Result<()> {
    let mut tx = pool.begin().await?;
    let domain_uid = insert_domain(&mut tx, "example.com").await?;
    let created_at = Utc::now();
    insert_certificate(
        &mut tx,
        domain_uid,
        created_at,
        created_at + Duration::from_hours(24),
    )
    .await?;

    retire_domain(&mut tx, domain_uid).await?;
    resurrect_domain(&mut tx, domain_uid).await?;
    tx.commit().await?;

    assert_eq!(select_latest_expiry_per_domain(&pool).await?.len(), 1);
    assert!(is_domain_active(&pool, domain_uid).await?);

    let mut tx = pool.begin().await?;
    retire_domain(&mut tx, domain_uid).await?;
    tx.commit().await?;

    assert!(select_latest_expiry_per_domain(&pool).await?.is_empty());
    assert!(!is_domain_active(&pool, domain_uid).await?);

    Ok(())
}

#[sqlx::test]
async fn can_select_domains_by_name_and_uid(pool: PgPool) -> Result<()> {
    let mut tx = pool.begin().await?;

    assert!(
        select_domain_by_name(&mut tx, "example.com")
            .await?
            .is_none()
    );

    let domain_uid = insert_domain(&mut tx, "example.com").await?;

    let record = select_domain_by_name(&mut tx, "example.com")
        .await?
        .unwrap();
    assert_eq!(*record.domain_uid, *domain_uid);
    assert_eq!(record.status, DomainStatus::Active);

    retire_domain(&mut tx, domain_uid).await?;

    let record = select_domain_by_uid(&mut tx, domain_uid).await?.unwrap();
    assert_eq!(record.name, "example.com");
    assert_eq!(record.status, DomainStatus::Retired);

    Ok(())
}
