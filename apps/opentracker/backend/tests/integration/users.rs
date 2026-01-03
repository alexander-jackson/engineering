use opentracker::persistence;
use sqlx::PgPool;

use crate::utils::*;

#[sqlx::test]
async fn users_can_be_created_and_found(pool: PgPool) -> sqlx::Result<()> {
    let id = some_user(&pool).await?;

    let found = persistence::account::find_by_email(SOME_EMAIL, &pool)
        .await?
        .unwrap();

    assert_eq!(id, found.account_uid);
    assert_eq!(SOME_EMAIL, found.email_address);
    assert_eq!(SOME_HASHED_PASSWORD, found.password);

    Ok(())
}

#[sqlx::test]
async fn non_existant_users_are_not_found(pool: PgPool) -> sqlx::Result<()> {
    let found = persistence::account::find_by_email(SOME_EMAIL, &pool).await?;

    assert!(found.is_none());

    Ok(())
}

#[sqlx::test]
async fn new_users_have_a_valid_created_at_timestamp(pool: PgPool) -> sqlx::Result<()> {
    some_user(&pool).await?;

    let found = persistence::account::find_by_email(SOME_EMAIL, &pool)
        .await?
        .unwrap();

    let now = chrono::Utc::now();

    assert!(found.created_at <= now);

    Ok(())
}

#[sqlx::test]
async fn email_search_is_case_insensitive(pool: PgPool) -> sqlx::Result<()> {
    persistence::account::insert(SOME_EMAIL, SOME_HASHED_PASSWORD, &pool).await?;

    let found = persistence::account::find_by_email(SOME_EQUIVALENT_EMAIL, &pool).await?;

    assert!(found.is_some());

    Ok(())
}

#[sqlx::test]
async fn users_cannot_be_created_with_the_same_email(pool: PgPool) -> sqlx::Result<()> {
    some_user(&pool).await?;

    let found = persistence::account::find_by_email(SOME_EMAIL, &pool).await?;

    assert!(found.is_some());

    // Create another user with the "same" email
    let result =
        persistence::account::insert(SOME_EQUIVALENT_EMAIL, SOME_HASHED_PASSWORD, &pool).await;

    assert!(result.is_err());

    Ok(())
}

#[sqlx::test]
async fn users_can_be_found_by_id(pool: PgPool) -> sqlx::Result<()> {
    let id = some_user(&pool).await?;

    let found = persistence::account::find_by_id(id, &pool).await?.unwrap();

    assert_eq!(id, found.account_uid);
    assert_eq!(SOME_EMAIL, found.email_address);
    assert_eq!(SOME_HASHED_PASSWORD, found.password);

    Ok(())
}

#[sqlx::test]
async fn passwords_can_be_updated(pool: PgPool) -> sqlx::Result<()> {
    let id = some_user(&pool).await?;

    // Update their password
    persistence::account::update_password(id, "<other>", &pool).await?;

    let user = persistence::account::find_by_id(id, &pool).await?.unwrap();

    assert_eq!(user.password, "<other>");

    Ok(())
}

#[sqlx::test]
async fn emails_are_not_initially_verified(pool: PgPool) -> sqlx::Result<()> {
    let id = some_user(&pool).await?;

    let status = persistence::account::fetch_email_verification_status(id, &pool).await?;

    assert!(status.verified_at.is_none());

    Ok(())
}

#[sqlx::test]
async fn emails_can_be_verified(pool: PgPool) -> sqlx::Result<()> {
    let id = some_user(&pool).await?;

    // Fetch their email address UID
    let email_address_uid = persistence::account::fetch_email_verification_status(id, &pool)
        .await?
        .email_address_uid;

    // Verify their email address
    persistence::account::verify_email(email_address_uid, &pool).await?;

    let status = persistence::account::fetch_email_verification_status(id, &pool).await?;

    assert!(status.verified_at.is_some());

    Ok(())
}
