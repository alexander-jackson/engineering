use chrono::Duration;
use sqlx::{pool::PoolConnection, Postgres};

use opentracker::forms::RecordedDate;
use opentracker::{forms, persistence};

use crate::utils::*;

#[sqlx::test]
async fn existing_workout_uids_are_used_if_they_exist(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;

    // Create a workout
    let recorded = date(15, 1, 2022);
    let first_uid = persistence::workouts::create_or_fetch(user_id, recorded, &mut conn).await?;

    // Create it again
    let second_uid = persistence::workouts::create_or_fetch(user_id, recorded, &mut conn).await?;

    assert_eq!(first_uid, second_uid);

    Ok(())
}

#[sqlx::test]
async fn workouts_can_have_exercises_and_be_queried(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;

    // Create a workout
    let recorded = date(15, 1, 2022);
    let id = persistence::workouts::create_or_fetch(user_id, recorded, &mut conn).await?;

    // Add some exercises to it
    let exercise = forms::Exercise {
        variant: forms::ExerciseVariant::Squat,
        description: String::from("Competition"),
        weight: 150.0,
        reps: 1,
        sets: 1,
        rpe: Some(7.5),
    };
    persistence::exercises::insert(id, &exercise, &mut conn).await?;

    // Fetch the workout itself
    let exercises =
        persistence::workouts::fetch_exercises_for_workout(user_id, recorded, &mut conn).await?;

    assert_eq!(exercises, vec![exercise]);

    Ok(())
}

#[sqlx::test]
async fn workout_exercises_can_be_queried(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;

    // Create a workout
    let recorded = date(15, 1, 2022);
    let id = persistence::workouts::create_or_fetch(user_id, recorded, &mut conn).await?;

    // Add some exercises to it
    let exercises = vec![
        forms::Exercise {
            variant: forms::ExerciseVariant::Squat,
            description: String::from("Competition"),
            weight: 150.0,
            reps: 1,
            sets: 1,
            rpe: Some(7.5),
        },
        forms::Exercise {
            variant: forms::ExerciseVariant::Bench,
            description: String::from("Competition"),
            weight: 93.0,
            reps: 1,
            sets: 1,
            rpe: None,
        },
    ];

    for exercise in &exercises {
        persistence::exercises::insert(id, exercise, &mut conn).await?;
    }

    // Fetch the workout itself
    let exercises = persistence::exercises::fetch_for_workout(id, &mut conn).await?;

    let expected_exercises = vec![
        forms::Exercise {
            variant: forms::ExerciseVariant::Squat,
            description: String::from("Competition"),
            weight: 150.0,
            reps: 1,
            sets: 1,
            rpe: Some(7.5),
        },
        forms::Exercise {
            variant: forms::ExerciseVariant::Bench,
            description: String::from("Competition"),
            weight: 93.0,
            reps: 1,
            sets: 1,
            rpe: None,
        },
    ];

    for (actual, expected) in exercises.iter().zip(expected_exercises.iter()) {
        assert_eq!(actual.variant, expected.variant);
        assert_eq!(actual.description, expected.description);
        assert_eq!(actual.weight, expected.weight);
        assert_eq!(actual.reps, expected.reps);
        assert_eq!(actual.sets, expected.sets);
        assert_eq!(actual.rpe, expected.rpe);
    }

    Ok(())
}

#[sqlx::test]
async fn all_workouts_can_be_fetched_for_a_user(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;

    // Create 2 workouts
    let first_date = date(15, 1, 2022);
    let first_id = persistence::workouts::create_or_fetch(user_id, first_date, &mut conn).await?;

    // Add some exercises to it
    let exercises = vec![
        forms::Exercise {
            variant: forms::ExerciseVariant::Squat,
            description: String::from("Competition"),
            weight: 150.0,
            reps: 1,
            sets: 1,
            rpe: Some(7.5),
        },
        forms::Exercise {
            variant: forms::ExerciseVariant::Bench,
            description: String::from("Competition"),
            weight: 93.0,
            reps: 1,
            sets: 1,
            rpe: None,
        },
    ];

    for exercise in &exercises {
        persistence::exercises::insert(first_id, exercise, &mut conn).await?;
    }

    let second_date = date(17, 1, 2022);
    let second_id = persistence::workouts::create_or_fetch(user_id, second_date, &mut conn).await?;

    // Add some exercises to it
    let exercises = vec![
        forms::Exercise {
            variant: forms::ExerciseVariant::Deadlift,
            description: String::from("Competition"),
            weight: 175.0,
            reps: 1,
            sets: 1,
            rpe: Some(8.5),
        },
        forms::Exercise {
            variant: forms::ExerciseVariant::Other,
            description: String::from("Barbell Row"),
            weight: 65.0,
            reps: 10,
            sets: 4,
            rpe: Some(7.5),
        },
    ];

    for exercise in &exercises {
        persistence::exercises::insert(second_id, exercise, &mut conn).await?;
    }

    let start = (first_date - Duration::days(1))
        .and_time(time(0, 0, 0))
        .and_utc();

    let end = (second_date + Duration::days(1))
        .and_time(time(0, 0, 0))
        .and_utc();

    // Fetch the workout itself
    let dated_exercises =
        persistence::workouts::fetch_with_exercises_between(user_id, start, end, &mut conn).await?;

    let expected_values = vec![
        persistence::workouts::DatedExercise {
            recorded: RecordedDate(second_date),
            variant: forms::ExerciseVariant::Deadlift,
            description: String::from("Competition"),
            weight: 175.0,
            reps: 1,
            sets: 1,
            rpe: Some(8.5),
        },
        persistence::workouts::DatedExercise {
            recorded: RecordedDate(second_date),
            variant: forms::ExerciseVariant::Other,
            description: String::from("Barbell Row"),
            weight: 65.0,
            reps: 10,
            sets: 4,
            rpe: Some(7.5),
        },
        persistence::workouts::DatedExercise {
            recorded: RecordedDate(first_date),
            variant: forms::ExerciseVariant::Squat,
            description: String::from("Competition"),
            weight: 150.0,
            reps: 1,
            sets: 1,
            rpe: Some(7.5),
        },
        persistence::workouts::DatedExercise {
            recorded: RecordedDate(first_date),
            variant: forms::ExerciseVariant::Bench,
            description: String::from("Competition"),
            weight: 93.0,
            reps: 1,
            sets: 1,
            rpe: None,
        },
    ];

    for (actual, expected) in dated_exercises.iter().zip(expected_values.iter()) {
        assert_eq!(actual.recorded, expected.recorded);
        assert_eq!(actual.variant, expected.variant);
        assert_eq!(actual.description, expected.description);
        assert_eq!(actual.weight, expected.weight);
        assert_eq!(actual.reps, expected.reps);
        assert_eq!(actual.sets, expected.sets);
        assert_eq!(actual.rpe, expected.rpe);
    }

    Ok(())
}

#[sqlx::test]
async fn workouts_can_be_deleted(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;

    // Create a workout
    let recorded = date(15, 1, 2022);
    let workout_uid = persistence::workouts::create_or_fetch(user_id, recorded, &mut conn).await?;

    // Add some exercises to it
    let exercise = forms::Exercise {
        variant: forms::ExerciseVariant::Squat,
        description: String::from("Competition"),
        weight: 150.0,
        reps: 1,
        sets: 1,
        rpe: Some(7.5),
    };
    persistence::exercises::insert(workout_uid, &exercise, &mut conn).await?;

    // Fetch the workout itself
    let exercises =
        persistence::workouts::fetch_exercises_for_workout(user_id, recorded, &mut conn).await?;

    // Ensure it exists
    assert!(!exercises.is_empty());

    // Delete it and make sure it doesn't exist anymore
    persistence::workouts::delete_by_date(user_id, recorded, &mut conn).await?;

    let exercises =
        persistence::workouts::fetch_exercises_for_workout(user_id, recorded, &mut conn).await?;

    assert!(exercises.is_empty());

    Ok(())
}
