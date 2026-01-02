use chrono::{Duration, Utc};
use float_cmp::approx_eq;
use sqlx::{Postgres, pool::PoolConnection};

use opentracker::forms::{ExerciseVariant, RecordedDate};
use opentracker::persistence::statistics::RepPersonalBest;
use opentracker::{forms, persistence};

use crate::utils::*;

#[sqlx::test]
async fn rep_personal_bests_can_be_queried(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;

    let first_workout_date = date(15, 1, 2022);
    let second_workout_date = date(17, 1, 2022);

    // Create some workouts
    let workouts = vec![
        (
            first_workout_date,
            vec![
                forms::Exercise {
                    variant: forms::ExerciseVariant::Deadlift,
                    description: String::from("Competition"),
                    weight: 175.0,
                    reps: 1,
                    sets: 1,
                    rpe: Some(8.5),
                },
                forms::Exercise {
                    variant: forms::ExerciseVariant::Deadlift,
                    description: String::from("Competition"),
                    weight: 145.0,
                    reps: 4,
                    sets: 3,
                    rpe: None,
                },
            ],
        ),
        (
            second_workout_date,
            vec![
                forms::Exercise {
                    variant: forms::ExerciseVariant::Deadlift,
                    description: String::from("Competition"),
                    weight: 165.0,
                    reps: 2,
                    sets: 1,
                    rpe: Some(9.0),
                },
                forms::Exercise {
                    variant: forms::ExerciseVariant::Deadlift,
                    description: String::from("Competition"),
                    weight: 135.0,
                    reps: 4,
                    sets: 5,
                    rpe: Some(6.5),
                },
            ],
        ),
    ];

    for (recorded, exercises) in workouts {
        // Create the workout
        let workout_id =
            persistence::workouts::create_or_fetch(user_id, recorded, &mut conn).await?;

        for exercise in exercises {
            persistence::exercises::insert(workout_id, &exercise, &mut conn).await?;
        }
    }

    // Query the unique exercises
    let rep_personal_bests = persistence::statistics::get_rep_personal_bests(
        user_id,
        ExerciseVariant::Deadlift,
        "Competition",
        &mut conn,
    )
    .await?;

    let expected_values = vec![
        RepPersonalBest {
            weight: 175.0,
            reps: 1,
            recorded: RecordedDate(first_workout_date),
        },
        RepPersonalBest {
            weight: 165.0,
            reps: 2,
            recorded: RecordedDate(second_workout_date),
        },
        RepPersonalBest {
            weight: 145.0,
            reps: 4,
            recorded: RecordedDate(first_workout_date),
        },
    ];

    for (actual, expected) in rep_personal_bests.iter().zip(expected_values.iter()) {
        assert_eq!(actual.weight, expected.weight);
        assert_eq!(actual.reps, expected.reps);
    }

    Ok(())
}

#[sqlx::test]
async fn rep_personal_bests_appear_once_per_rep_count(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;
    let workout_date = date(15, 1, 2022);

    // Create some workouts
    let workouts = vec![
        (
            workout_date,
            vec![forms::Exercise {
                variant: forms::ExerciseVariant::Deadlift,
                description: String::from("Competition"),
                weight: 175.0,
                reps: 1,
                sets: 1,
                rpe: Some(8.5),
            }],
        ),
        (
            workout_date - Duration::days(1),
            vec![forms::Exercise {
                variant: forms::ExerciseVariant::Deadlift,
                description: String::from("Competition"),
                weight: 170.0,
                reps: 1,
                sets: 1,
                rpe: Some(8.0),
            }],
        ),
    ];

    for (recorded, exercises) in workouts {
        // Create the workout
        let workout_id =
            persistence::workouts::create_or_fetch(user_id, recorded, &mut conn).await?;

        for exercise in exercises {
            persistence::exercises::insert(workout_id, &exercise, &mut conn).await?;
        }
    }

    // Query the rep PBs
    let rep_personal_bests = persistence::statistics::get_rep_personal_bests(
        user_id,
        ExerciseVariant::Deadlift,
        "Competition",
        &mut conn,
    )
    .await?;

    let expected_values = vec![RepPersonalBest {
        weight: 175.0,
        reps: 1,
        recorded: RecordedDate(workout_date),
    }];

    // We should have the same number of entries
    assert_eq!(rep_personal_bests.len(), expected_values.len());

    for (actual, expected) in rep_personal_bests.iter().zip(expected_values.iter()) {
        assert_eq!(actual.weight, expected.weight);
        assert_eq!(actual.reps, expected.reps);
    }

    Ok(())
}

#[sqlx::test]
async fn rep_personal_bests_appear_once_per_weight_and_rep_combination(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;
    let workout_date = date(15, 1, 2022);

    // Create some workouts
    let workouts = vec![
        (
            workout_date,
            vec![forms::Exercise {
                variant: forms::ExerciseVariant::Deadlift,
                description: String::from("Competition"),
                weight: 175.0,
                reps: 1,
                sets: 1,
                rpe: None,
            }],
        ),
        (
            workout_date - Duration::days(1),
            vec![forms::Exercise {
                variant: forms::ExerciseVariant::Deadlift,
                description: String::from("Competition"),
                weight: 175.0,
                reps: 1,
                sets: 1,
                rpe: None,
            }],
        ),
    ];

    for (recorded, exercises) in workouts {
        // Create the workout
        let workout_id =
            persistence::workouts::create_or_fetch(user_id, recorded, &mut conn).await?;

        for exercise in exercises {
            persistence::exercises::insert(workout_id, &exercise, &mut conn).await?;
        }
    }

    // Query the rep PBs
    let rep_personal_bests = persistence::statistics::get_rep_personal_bests(
        user_id,
        ExerciseVariant::Deadlift,
        "Competition",
        &mut conn,
    )
    .await?;

    let expected_values = vec![RepPersonalBest {
        weight: 175.0,
        reps: 1,
        recorded: RecordedDate(workout_date),
    }];

    // We should have the same number of entries
    assert_eq!(rep_personal_bests.len(), expected_values.len());

    for (actual, expected) in rep_personal_bests.iter().zip(expected_values.iter()) {
        assert_eq!(actual.weight, expected.weight);
        assert_eq!(actual.reps, expected.reps);
    }

    Ok(())
}

#[sqlx::test]
async fn bodyweight_deltas_can_be_queried(mut conn: PoolConnection<Postgres>) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;

    let current_bodyweight = 82.5;
    let one_week_delta = 1.2;
    let four_week_delta = -0.3;
    let current_date = Utc::now().date_naive();
    let one_week_ago = current_date - chrono::Duration::days(6);
    let four_weeks_ago = current_date - chrono::Duration::weeks(4);

    // Enter some bodyweight values
    persistence::bodyweights::insert(user_id, current_bodyweight, current_date, &mut conn).await?;

    persistence::bodyweights::insert(
        user_id,
        current_bodyweight - one_week_delta,
        one_week_ago,
        &mut conn,
    )
    .await?;

    persistence::bodyweights::insert(
        user_id,
        current_bodyweight - four_week_delta,
        four_weeks_ago,
        &mut conn,
    )
    .await?;

    // Query the statistics
    let stats = persistence::statistics::get_bodyweight_statistics(user_id, &mut conn).await?;

    // Check the values are approximately what we want
    assert!(approx_eq!(
        f32,
        stats.increase_last_week.unwrap(),
        one_week_delta,
        epsilon = 0.0001
    ));

    assert!(approx_eq!(
        f32,
        stats.increase_last_month.unwrap(),
        four_week_delta,
        epsilon = 0.0001
    ));

    Ok(())
}

#[sqlx::test]
async fn bodyweight_averages_can_be_queried(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;

    let current_bodyweight = 82.5;
    let current_date = Utc::now().date_naive();

    // Enter their current bodyweight
    persistence::bodyweights::insert(user_id, current_bodyweight, current_date, &mut conn).await?;

    // Make some entries for the last week
    persistence::bodyweights::insert(
        user_id,
        current_bodyweight - 0.3,
        current_date - chrono::Duration::days(2),
        &mut conn,
    )
    .await?;

    persistence::bodyweights::insert(
        user_id,
        current_bodyweight - 0.6,
        current_date - chrono::Duration::days(4),
        &mut conn,
    )
    .await?;

    // Make some entries from a few weeks ago
    persistence::bodyweights::insert(
        user_id,
        current_bodyweight + 0.1,
        current_date - chrono::Duration::weeks(2),
        &mut conn,
    )
    .await?;

    persistence::bodyweights::insert(
        user_id,
        current_bodyweight + 0.4,
        current_date - chrono::Duration::weeks(3),
        &mut conn,
    )
    .await?;

    persistence::bodyweights::insert(
        user_id,
        current_bodyweight + 0.1,
        current_date - chrono::Duration::weeks(3) - chrono::Duration::days(2),
        &mut conn,
    )
    .await?;

    // Query the statistics
    let stats = persistence::statistics::get_bodyweight_statistics(user_id, &mut conn).await?;

    // Check the values are approximately what we want
    assert!(approx_eq!(
        f64,
        stats.average_last_week.unwrap(),
        82.2,
        epsilon = 0.0001
    ));

    assert!(approx_eq!(
        f64,
        stats.average_last_month.unwrap(),
        82.45,
        epsilon = 0.0001
    ));

    Ok(())
}

#[sqlx::test]
async fn weekly_workout_volumes_can_be_queried(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;

    let current_date = Utc::now().date_naive();

    // Record a workout for them
    let workout_id =
        persistence::workouts::create_or_fetch(user_id, current_date, &mut conn).await?;

    // Add some exercises to it
    let exercises = vec![
        forms::Exercise {
            variant: forms::ExerciseVariant::Squat,
            description: String::from("Competition"),
            weight: 160.0,
            reps: 4,
            sets: 3,
            rpe: Some(7.5),
        },
        forms::Exercise {
            variant: forms::ExerciseVariant::Bench,
            description: String::from("Spoto"),
            weight: 93.0,
            reps: 2,
            sets: 2,
            rpe: None,
        },
        forms::Exercise {
            variant: forms::ExerciseVariant::Other,
            description: String::from("Hammer Curl"),
            weight: 16.0,
            reps: 12,
            sets: 4,
            rpe: None,
        },
    ];

    for exercise in &exercises {
        persistence::exercises::insert(workout_id, exercise, &mut conn).await?;
    }

    // Get the statistics
    let stats =
        persistence::statistics::get_workout_statistics(user_id, current_date, &mut conn).await?;

    assert!(approx_eq!(
        f64,
        stats.squat_volume_past_week.unwrap(),
        160.0 * 4.0 * 3.0,
        epsilon = 0.01
    ));

    assert!(approx_eq!(
        f64,
        stats.bench_volume_past_week.unwrap(),
        93.0 * 2.0 * 2.0,
        epsilon = 0.01
    ));

    assert!(approx_eq!(
        f64,
        stats.other_volume_past_week.unwrap(),
        16.0 * 12.0 * 4.0,
        epsilon = 0.01
    ));

    assert!(stats.deadlift_volume_past_week.is_none());

    // Check for ages ago
    let stats = persistence::statistics::get_workout_statistics(
        user_id,
        current_date - Duration::weeks(4),
        &mut conn,
    )
    .await?;

    assert!(stats.squat_volume_past_week.is_none());
    assert!(stats.bench_volume_past_week.is_none());
    assert!(stats.deadlift_volume_past_week.is_none());

    Ok(())
}

#[sqlx::test]
async fn users_cannot_see_each_others_stats(
    mut conn: PoolConnection<Postgres>,
) -> sqlx::Result<()> {
    // Create a user
    let user_id = some_user(&mut conn).await?;

    let current_date = Utc::now().date_naive();

    // Record a workout for them
    let workout_id =
        persistence::workouts::create_or_fetch(user_id, current_date, &mut conn).await?;

    // Add some exercises to it
    let exercises = vec![
        forms::Exercise {
            variant: forms::ExerciseVariant::Squat,
            description: String::from("Competition"),
            weight: 160.0,
            reps: 4,
            sets: 3,
            rpe: Some(7.5),
        },
        forms::Exercise {
            variant: forms::ExerciseVariant::Bench,
            description: String::from("Spoto"),
            weight: 93.0,
            reps: 2,
            sets: 2,
            rpe: None,
        },
    ];

    for exercise in &exercises {
        persistence::exercises::insert(workout_id, exercise, &mut conn).await?;
    }

    // Create another user
    let second_user = persistence::account::insert("alex@foobar.com", "foobar", &mut conn).await?;

    // Get the statistics
    let stats =
        persistence::statistics::get_workout_statistics(second_user, current_date, &mut conn)
            .await?;

    assert!(stats.squat_volume_past_week.is_none());
    assert!(stats.bench_volume_past_week.is_none());
    assert!(stats.deadlift_volume_past_week.is_none());

    Ok(())
}
