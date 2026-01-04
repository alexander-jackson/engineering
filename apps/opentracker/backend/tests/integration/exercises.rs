use opentracker::{forms, persistence};
use sqlx::PgPool;

use crate::utils::*;

#[sqlx::test]
async fn unique_exercises_can_be_queried(pool: PgPool) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&pool).await?;

    // Create some workouts
    let workouts = vec![
        (
            date(15, 1, 2022),
            vec![
                forms::Exercise {
                    variant: forms::ExerciseVariant::Bench,
                    description: String::from("Competition"),
                    weight: 122.5,
                    reps: 1,
                    sets: 1,
                    rpe: Some(8.0),
                },
                forms::Exercise {
                    variant: forms::ExerciseVariant::Deadlift,
                    description: String::from("Competition"),
                    weight: 175.0,
                    reps: 1,
                    sets: 1,
                    rpe: None,
                },
            ],
        ),
        (
            date(17, 1, 2022),
            vec![forms::Exercise {
                variant: forms::ExerciseVariant::Bench,
                description: String::from("Spoto"),
                weight: 95.0,
                reps: 7,
                sets: 4,
                rpe: Some(7.5),
            }],
        ),
    ];

    for (recorded, exercises) in workouts {
        // Create the workout
        let workout_id = persistence::workouts::create_or_fetch(user_id, recorded, &pool).await?;

        for exercise in exercises {
            persistence::exercises::insert(workout_id, &exercise, &pool).await?;
        }
    }

    // Query the unique exercises
    let bench_variations =
        persistence::exercises::fetch_unique(user_id, forms::ExerciseVariant::Bench, &pool).await?;

    assert_eq!(bench_variations, &["Competition", "Spoto"]);

    let deadlift_variations =
        persistence::exercises::fetch_unique(user_id, forms::ExerciseVariant::Deadlift, &pool)
            .await?;

    assert_eq!(deadlift_variations, &["Competition"]);

    Ok(())
}

#[sqlx::test]
async fn workout_exercises_can_be_deleted(pool: PgPool) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&pool).await?;

    // Create a workout
    let recorded = date(15, 1, 2022);
    let workout_id = persistence::workouts::create_or_fetch(user_id, recorded, &pool).await?;

    // Add some exercises
    let exercises = vec![
        forms::Exercise {
            variant: forms::ExerciseVariant::Bench,
            description: String::from("Competition"),
            weight: 122.5,
            reps: 1,
            sets: 1,
            rpe: Some(7.5),
        },
        forms::Exercise {
            variant: forms::ExerciseVariant::Bench,
            description: String::from("Competition"),
            weight: 110.0,
            reps: 4,
            sets: 3,
            rpe: Some(8.0),
        },
    ];

    for exercise in &exercises {
        persistence::exercises::insert(workout_id, exercise, &pool).await?;
    }

    // Check we have some
    let exercises = persistence::exercises::fetch_for_workout(workout_id, &pool).await?;

    assert!(exercises.len() > 0);

    // Delete and check we have none
    persistence::exercises::delete_by_date(user_id, recorded, &pool).await?;
    let exercises = persistence::exercises::fetch_for_workout(workout_id, &pool).await?;

    assert!(exercises.is_empty());

    Ok(())
}

#[sqlx::test]
async fn exercises_can_be_renamed(pool: PgPool) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&pool).await?;

    // Create a workout
    let recorded = date(15, 1, 2022);
    let workout_id = persistence::workouts::create_or_fetch(user_id, recorded, &pool).await?;

    // Add some exercises
    let exercises = vec![
        forms::Exercise {
            variant: forms::ExerciseVariant::Bench,
            description: String::from("Competition"),
            weight: 122.5,
            reps: 1,
            sets: 1,
            rpe: Some(7.5),
        },
        forms::Exercise {
            variant: forms::ExerciseVariant::Bench,
            description: String::from("3 Count Paused"),
            weight: 110.0,
            reps: 4,
            sets: 3,
            rpe: Some(8.0),
        },
    ];

    for exercise in &exercises {
        persistence::exercises::insert(workout_id, exercise, &pool).await?;
    }

    // Rename `3 Count Paused` to `3ct Pause`
    let result = persistence::exercises::rename(
        user_id,
        forms::ExerciseVariant::Bench,
        "3 Count Paused",
        "3ct Pause",
        &pool,
    )
    .await?;

    // Check we updated one row
    assert_eq!(result, 1);

    // Check the exercises for the workout look as expected
    let exercises = persistence::exercises::fetch_for_workout(workout_id, &pool).await?;

    let expected_exercises = vec![
        forms::Exercise {
            variant: forms::ExerciseVariant::Bench,
            description: String::from("Competition"),
            weight: 122.5,
            reps: 1,
            sets: 1,
            rpe: Some(7.5),
        },
        forms::Exercise {
            variant: forms::ExerciseVariant::Bench,
            description: String::from("3ct Pause"),
            weight: 110.0,
            reps: 4,
            sets: 3,
            rpe: Some(8.0),
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
async fn last_session_can_be_fetched_for_structured_exercises(pool: PgPool) -> sqlx::Result<()> {
    let user_id = some_user(&pool).await?;

    // Create two workouts with the same exercise on different dates
    let first_date = date(10, 1, 2024);
    let second_date = date(15, 1, 2024);

    let first_workout_id =
        persistence::workouts::create_or_fetch(user_id, first_date, &pool).await?;
    let second_workout_id =
        persistence::workouts::create_or_fetch(user_id, second_date, &pool).await?;

    let first_exercise = forms::Exercise {
        variant: forms::ExerciseVariant::Squat,
        description: String::from("Competition"),
        weight: 100.0,
        reps: 5,
        sets: 3,
        rpe: Some(7.0),
    };

    let second_exercise = forms::Exercise {
        variant: forms::ExerciseVariant::Squat,
        description: String::from("Competition"),
        weight: 105.0,
        reps: 5,
        sets: 3,
        rpe: Some(7.5),
    };

    persistence::exercises::insert(first_workout_id, &first_exercise, &pool).await?;
    persistence::exercises::insert(second_workout_id, &second_exercise, &pool).await?;

    // Fetch the last session for a new workout on Jan 20
    let current_date = date(20, 1, 2024);
    let result = persistence::exercises::fetch_last_session(
        user_id,
        forms::ExerciseVariant::Squat,
        "Competition",
        current_date,
        &pool,
    )
    .await?;

    // Should return the second exercise (most recent before current date)
    assert!(result.is_some());
    let (exercise, recorded_date) = result.unwrap();
    assert_eq!(exercise.weight, 105.0);
    assert_eq!(exercise.reps, 5);
    assert_eq!(exercise.sets, 3);
    assert_eq!(exercise.rpe, Some(7.5));
    assert_eq!(recorded_date, second_date);

    Ok(())
}

#[sqlx::test]
async fn last_session_returns_none_when_no_previous_session_exists(
    pool: PgPool,
) -> sqlx::Result<()> {
    let user_id = some_user(&pool).await?;

    // Query for a session that doesn't exist
    let current_date = date(20, 1, 2024);
    let result = persistence::exercises::fetch_last_session(
        user_id,
        forms::ExerciseVariant::Bench,
        "Competition",
        current_date,
        &pool,
    )
    .await?;

    assert!(result.is_none());

    Ok(())
}

#[sqlx::test]
async fn last_session_excludes_current_date(pool: PgPool) -> sqlx::Result<()> {
    let user_id = some_user(&pool).await?;

    let first_date = date(10, 1, 2024);
    let second_date = date(15, 1, 2024);

    let first_workout_id =
        persistence::workouts::create_or_fetch(user_id, first_date, &pool).await?;
    let second_workout_id =
        persistence::workouts::create_or_fetch(user_id, second_date, &pool).await?;

    let first_exercise = forms::Exercise {
        variant: forms::ExerciseVariant::Deadlift,
        description: String::from("Competition"),
        weight: 150.0,
        reps: 3,
        sets: 3,
        rpe: Some(8.0),
    };

    let second_exercise = forms::Exercise {
        variant: forms::ExerciseVariant::Deadlift,
        description: String::from("Competition"),
        weight: 160.0,
        reps: 3,
        sets: 3,
        rpe: Some(8.5),
    };

    persistence::exercises::insert(first_workout_id, &first_exercise, &pool).await?;
    persistence::exercises::insert(second_workout_id, &second_exercise, &pool).await?;

    // Fetch last session using the second date as current date
    // Should return the first exercise, not the second
    let result = persistence::exercises::fetch_last_session(
        user_id,
        forms::ExerciseVariant::Deadlift,
        "Competition",
        second_date,
        &pool,
    )
    .await?;

    assert!(result.is_some());
    let (exercise, recorded_date) = result.unwrap();
    assert_eq!(exercise.weight, 150.0);
    assert_eq!(recorded_date, first_date);

    Ok(())
}

#[sqlx::test]
async fn last_session_can_be_fetched_for_freeform_exercises(pool: PgPool) -> sqlx::Result<()> {
    let user_id = some_user(&pool).await?;

    let first_date = date(10, 1, 2024);
    let second_date = date(15, 1, 2024);

    let first_workout_id =
        persistence::workouts::create_or_fetch(user_id, first_date, &pool).await?;
    let second_workout_id =
        persistence::workouts::create_or_fetch(user_id, second_date, &pool).await?;

    let first_exercise = forms::Exercise {
        variant: forms::ExerciseVariant::Other,
        description: String::from("Barbell Row"),
        weight: 80.0,
        reps: 8,
        sets: 4,
        rpe: Some(7.0),
    };

    let second_exercise = forms::Exercise {
        variant: forms::ExerciseVariant::Other,
        description: String::from("Barbell Row"),
        weight: 85.0,
        reps: 8,
        sets: 4,
        rpe: Some(7.5),
    };

    persistence::exercises::insert(first_workout_id, &first_exercise, &pool).await?;
    persistence::exercises::insert(second_workout_id, &second_exercise, &pool).await?;

    // Fetch the last session
    let current_date = date(20, 1, 2024);
    let result = persistence::exercises::fetch_last_session(
        user_id,
        forms::ExerciseVariant::Other,
        "Barbell Row",
        current_date,
        &pool,
    )
    .await?;

    assert!(result.is_some());
    let (exercise, recorded_date) = result.unwrap();
    assert_eq!(exercise.weight, 85.0);
    assert_eq!(recorded_date, second_date);

    Ok(())
}

#[sqlx::test]
async fn last_session_only_returns_matching_exercise_variant(pool: PgPool) -> sqlx::Result<()> {
    let user_id = some_user(&pool).await?;

    let workout_date = date(10, 1, 2024);
    let workout_id = persistence::workouts::create_or_fetch(user_id, workout_date, &pool).await?;

    // Add a squat exercise
    let squat_exercise = forms::Exercise {
        variant: forms::ExerciseVariant::Squat,
        description: String::from("Competition"),
        weight: 100.0,
        reps: 5,
        sets: 3,
        rpe: Some(7.0),
    };

    persistence::exercises::insert(workout_id, &squat_exercise, &pool).await?;

    // Query for bench with same description - should return None
    let current_date = date(15, 1, 2024);
    let result = persistence::exercises::fetch_last_session(
        user_id,
        forms::ExerciseVariant::Bench,
        "Competition",
        current_date,
        &pool,
    )
    .await?;

    assert!(result.is_none());

    Ok(())
}
