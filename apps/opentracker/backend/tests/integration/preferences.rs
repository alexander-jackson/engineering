use sqlx::{pool::PoolConnection, Postgres};
use uuid::Uuid;

use opentracker::persistence::{
    self,
    preferences::{Preferences, RepSetNotation},
};

#[sqlx::test]
async fn no_preferences_are_found_for_non_existant_users(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    let user_id = Uuid::new_v4();
    let preferences = persistence::preferences::fetch(user_id, &mut conn).await?;

    assert!(preferences.is_none());

    Ok(())
}

#[sqlx::test]
async fn preferences_can_be_inserted_and_fetched_for_users(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    let preferences = Preferences::new(RepSetNotation::SetsThenReps);

    // Create a new user
    let user_id = persistence::account::insert("some@email.com", "<hashed>", &mut conn).await?;

    // Add some preferences for them
    persistence::preferences::update(user_id, preferences, &mut conn).await?;

    // Fetch their preferences
    let persisted = persistence::preferences::fetch(user_id, &mut conn).await?;

    assert_eq!(Some(preferences), persisted);

    Ok(())
}

#[sqlx::test]
async fn preferences_can_be_updated(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    let initial_preferences = Preferences::new(RepSetNotation::SetsThenReps);
    let updated_preferences = Preferences::new(RepSetNotation::RepsThenSets);

    // Create a new user
    let user_id = persistence::account::insert("some@email.com", "<hashed>", &mut conn).await?;

    // Add some initial preferences for them
    persistence::preferences::update(user_id, initial_preferences, &mut conn).await?;

    // Update the persisted values
    persistence::preferences::update(user_id, updated_preferences, &mut conn).await?;

    // Fetch their preferences
    let persisted = persistence::preferences::fetch(user_id, &mut conn).await?;

    assert_ne!(Some(initial_preferences), persisted);
    assert_eq!(Some(updated_preferences), persisted);

    Ok(())
}
