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
        let workout_id =
            persistence::workouts::create_or_fetch(user_id, recorded, &pool).await?;

        for exercise in exercises {
            persistence::exercises::insert(workout_id, &exercise, &pool).await?;
        }
    }

    // Query the unique exercises
    let bench_variations =
        persistence::exercises::fetch_unique(user_id, forms::ExerciseVariant::Bench, &pool)
            .await?;

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
