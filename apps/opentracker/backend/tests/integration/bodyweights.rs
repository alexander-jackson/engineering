use chrono::NaiveDate;
use float_cmp::approx_eq;

use opentracker::persistence;
use sqlx::PgPool;
use uuid::Uuid;

use crate::utils::*;

#[sqlx::test]
async fn bodyweights_can_be_inserted_and_queried(pool: PgPool) -> sqlx::Result<()> {
    // Create a user
    let id = some_user(&pool).await?;

    let (bodyweight, recorded) = (82.5, date(12, 1, 2022));

    // Insert a bodyweight value
    persistence::bodyweights::insert(id, bodyweight, recorded, &pool).await?;

    // Fetch it from the database again
    let record = persistence::bodyweights::fetch_by_date(id, recorded, &pool)
        .await?
        .expect("Failed to find a record for this date")
        .bodyweight;

    approx_eq!(f32, record, bodyweight, ulps = 2);

    Ok(())
}

#[sqlx::test]
async fn non_existant_records_are_not_found(pool: PgPool) -> sqlx::Result<()> {
    // Create a user
    let id = some_user(&pool).await?;

    let recorded = date(12, 1, 2022);

    // This one hasn't been inserted this time
    let record = persistence::bodyweights::fetch_by_date(id, recorded, &pool).await?;

    assert!(record.is_none());

    Ok(())
}

#[sqlx::test]
async fn users_cannot_see_other_user_bodyweights(pool: PgPool) -> sqlx::Result<()> {
    // Create 2 users
    let first_id = persistence::account::insert("f@one.com", "", &pool).await?;
    let second_id = persistence::account::insert("f@two.com", "", &pool).await?;

    let (bodyweight, recorded) = (82.5, date(12, 1, 2022));

    // Insert a value for the first user
    persistence::bodyweights::insert(first_id, bodyweight, recorded, &pool).await?;

    // Fetch the same date for the second user
    let record = persistence::bodyweights::fetch_by_date(second_id, recorded, &pool).await?;

    assert!(record.is_none());

    Ok(())
}

#[sqlx::test]
async fn users_can_fetch_all_bodyweights(pool: PgPool) -> sqlx::Result<()> {
    // Create a user
    let id = some_user(&pool).await?;

    let values = vec![(82.5, date(12, 1, 2022)), (82.7, date(15, 1, 2022))];

    // Insert the values into the database
    for (bodyweight, recorded) in &values {
        persistence::bodyweights::insert(id, *bodyweight, *recorded, &pool).await?;
    }

    // Fetch all the data and check it is the same
    let records = persistence::bodyweights::fetch_all(id, &pool).await?;

    for (actual, expected) in records.iter().zip(values.iter()) {
        approx_eq!(f32, actual.bodyweight, expected.0, ulps = 2);
        assert_eq!(actual.recorded.0, expected.1);
    }

    Ok(())
}

#[sqlx::test]
async fn bodyweights_can_be_deleted(pool: PgPool) -> sqlx::Result<()> {
    // Create a user
    let id = some_user(&pool).await?;

    let values = vec![(82.5, date(12, 1, 2022)), (82.7, date(15, 1, 2022))];

    // Insert the values into the database
    for (bodyweight, recorded) in &values {
        persistence::bodyweights::insert(id, *bodyweight, *recorded, &pool).await?;
    }

    // Delete the bodyweight for the first day
    persistence::bodyweights::delete_by_date(id, values[0].1, &pool).await?;

    // Check it was deleted but the other was not
    assert!(
        persistence::bodyweights::fetch_by_date(id, values[0].1, &pool)
            .await?
            .is_none()
    );

    assert!(
        persistence::bodyweights::fetch_by_date(id, values[1].1, &pool)
            .await?
            .is_some()
    );

    Ok(())
}

async fn test_most_recent(
    user_id: Uuid,
    pool: &PgPool,
    expected: Option<(f32, NaiveDate)>,
) -> sqlx::Result<()> {
    let record = persistence::bodyweights::fetch_most_recent(user_id, pool).await?;
    let mapped = record.map(|value| (value.bodyweight, value.recorded.0));

    assert_eq!(mapped, expected);

    Ok(())
}

#[sqlx::test]
async fn most_recent_bodyweight_can_be_fetched(pool: PgPool) -> sqlx::Result<()> {
    let first_user = some_user(&pool).await?;
    let second_user = persistence::account::insert("user@foo.com", "something", &pool).await?;
    let third_user = persistence::account::insert("user@bar.com", "something", &pool).await?;

    // Insert some data
    persistence::bodyweights::insert(first_user, 82.5, date(1, 1, 2023), &pool).await?;
    persistence::bodyweights::insert(first_user, 82.3, date(3, 1, 2023), &pool).await?;
    persistence::bodyweights::insert(second_user, 113.1, date(5, 1, 2023), &pool).await?;

    test_most_recent(first_user, &pool, Some((82.3, date(3, 1, 2023)))).await?;
    test_most_recent(second_user, &pool, Some((113.1, date(5, 1, 2023)))).await?;
    test_most_recent(third_user, &pool, None).await?;

    Ok(())
}
