use opentracker::persistence;
use sqlx::Postgres;
use sqlx::pool::PoolConnection;

use crate::utils::*;

#[sqlx::test]
async fn users_can_be_created_and_found(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    let id = some_user(&mut conn).await?;

    let found = persistence::account::find_by_email(SOME_EMAIL, &mut conn)
        .await?
        .unwrap();

    assert_eq!(id, found.account_uid);
    assert_eq!(SOME_EMAIL, found.email_address);
    assert_eq!(SOME_HASHED_PASSWORD, found.password);

    Ok(())
}

#[sqlx::test]
async fn non_existant_users_are_not_found(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    let found = persistence::account::find_by_email(SOME_EMAIL, &mut conn).await?;

    assert!(found.is_none());

    Ok(())
}

#[sqlx::test]
async fn new_users_have_a_valid_created_at_timestamp(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    some_user(&mut conn).await?;

    let found = persistence::account::find_by_email(SOME_EMAIL, &mut conn)
        .await?
        .unwrap();

    let now = chrono::Utc::now();

    assert!(found.created_at <= now);

    Ok(())
}

#[sqlx::test]
async fn email_search_is_case_insensitive(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    persistence::account::insert(SOME_EMAIL, SOME_HASHED_PASSWORD, &mut conn).await?;

    let found = persistence::account::find_by_email(SOME_EQUIVALENT_EMAIL, &mut conn).await?;

    assert!(found.is_some());

    Ok(())
}

#[sqlx::test]
async fn users_cannot_be_created_with_the_same_email(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    some_user(&mut conn).await?;

    let found = persistence::account::find_by_email(SOME_EMAIL, &mut conn).await?;

    assert!(found.is_some());

    // Create another user with the "same" email
    let result =
        persistence::account::insert(SOME_EQUIVALENT_EMAIL, SOME_HASHED_PASSWORD, &mut conn).await;

    assert!(result.is_err());

    Ok(())
}

#[sqlx::test]
async fn users_can_be_found_by_id(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    let id = some_user(&mut conn).await?;

    let found = persistence::account::find_by_id(id, &mut conn)
        .await?
        .unwrap();

    assert_eq!(id, found.account_uid);
    assert_eq!(SOME_EMAIL, found.email_address);
    assert_eq!(SOME_HASHED_PASSWORD, found.password);

    Ok(())
}

#[sqlx::test]
async fn passwords_can_be_updated(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    let id = some_user(&mut conn).await?;

    // Update their password
    persistence::account::update_password(id, "<other>", &mut conn).await?;

    let user = persistence::account::find_by_id(id, &mut conn)
        .await?
        .unwrap();

    assert_eq!(user.password, "<other>");

    Ok(())
}

#[sqlx::test]
async fn emails_are_not_initially_verified(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    let id = some_user(&mut conn).await?;

    let status = persistence::account::fetch_email_verification_status(id, &mut conn).await?;

    assert!(status.verified_at.is_none());

    Ok(())
}

#[sqlx::test]
async fn emails_can_be_verified(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    let id = some_user(&mut conn).await?;

    // Fetch their email address UID
    let email_address_uid = persistence::account::fetch_email_verification_status(id, &mut conn)
        .await?
        .email_address_uid;

    // Verify their email address
    persistence::account::verify_email(email_address_uid, &mut conn).await?;

    let status = persistence::account::fetch_email_verification_status(id, &mut conn).await?;

    assert!(status.verified_at.is_some());

    Ok(())
}
