use itertools::Itertools;

use crate::forms;
use crate::persistence::workouts::{DatedExercise, DatedWorkout};

pub fn group_by_date(dated_exercises: Vec<DatedExercise>) -> Vec<DatedWorkout> {
    dated_exercises
        .into_iter()
        .batching(|v| match v.next() {
            None => None,
            Some(x) => {
                let recorded = x.recorded;
                let exercises = std::iter::once(x)
                    .chain(v.take_while_ref(|y| recorded == y.recorded))
                    .map(forms::Exercise::from)
                    .collect_vec();

                let dated = DatedWorkout {
                    recorded,
                    exercises,
                };

                Some(dated)
            }
        })
        .collect_vec()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    use crate::forms;

    #[test]
    fn single_dated_exercises_are_grouped_properly() {
        let first_day = forms::RecordedDate(NaiveDate::from_ymd_opt(2021, 10, 29).unwrap());

        let dated_exercises = vec![DatedExercise {
            recorded: first_day,
            variant: forms::ExerciseVariant::Bench,
            description: String::from("Competition"),
            weight: 110.0,
            reps: 3,
            sets: 3,
            rpe: None,
        }];

        let grouped = group_by_date(dated_exercises);

        let expected = vec![DatedWorkout {
            recorded: first_day,
            exercises: vec![forms::Exercise {
                variant: forms::ExerciseVariant::Bench,
                description: String::from("Competition"),
                weight: 110.0,
                reps: 3,
                sets: 3,
                rpe: None,
            }],
        }];

        assert_eq!(grouped, expected);
    }

    #[test]
    fn multiple_same_day_exercises_are_grouped_together() {
        let first_day = forms::RecordedDate(NaiveDate::from_ymd_opt(2021, 10, 29).unwrap());

        let dated_exercises = vec![
            DatedExercise {
                recorded: first_day,
                variant: forms::ExerciseVariant::Bench,
                description: String::from("Competition"),
                weight: 110.0,
                reps: 3,
                sets: 3,
                rpe: None,
            },
            DatedExercise {
                recorded: first_day,
                variant: forms::ExerciseVariant::Squat,
                description: String::from("Competition"),
                weight: 140.0,
                reps: 3,
                sets: 3,
                rpe: Some(8.5),
            },
        ];

        let grouped = group_by_date(dated_exercises);

        let expected = vec![DatedWorkout {
            recorded: first_day,
            exercises: vec![
                forms::Exercise {
                    variant: forms::ExerciseVariant::Bench,
                    description: String::from("Competition"),
                    weight: 110.0,
                    reps: 3,
                    sets: 3,
                    rpe: None,
                },
                forms::Exercise {
                    variant: forms::ExerciseVariant::Squat,
                    description: String::from("Competition"),
                    weight: 140.0,
                    reps: 3,
                    sets: 3,
                    rpe: Some(8.5),
                },
            ],
        }];

        assert_eq!(grouped, expected);
    }

    #[test]
    fn exercises_over_multiple_days_are_grouped_correctly() {
        let first_day = forms::RecordedDate(NaiveDate::from_ymd_opt(2021, 10, 29).unwrap());
        let second_day = forms::RecordedDate(NaiveDate::from_ymd_opt(2021, 10, 31).unwrap());

        let dated_exercises = vec![
            DatedExercise {
                recorded: first_day,
                variant: forms::ExerciseVariant::Bench,
                description: String::from("Competition"),
                weight: 110.0,
                reps: 3,
                sets: 3,
                rpe: None,
            },
            DatedExercise {
                recorded: first_day,
                variant: forms::ExerciseVariant::Squat,
                description: String::from("Competition"),
                weight: 140.0,
                reps: 3,
                sets: 3,
                rpe: Some(8.5),
            },
            DatedExercise {
                recorded: second_day,
                variant: forms::ExerciseVariant::Deadlift,
                description: String::from("Competition"),
                weight: 160.0,
                reps: 3,
                sets: 3,
                rpe: Some(7.5),
            },
        ];

        let grouped = group_by_date(dated_exercises);

        let expected = vec![
            DatedWorkout {
                recorded: first_day,
                exercises: vec![
                    forms::Exercise {
                        variant: forms::ExerciseVariant::Bench,
                        description: String::from("Competition"),
                        weight: 110.0,
                        reps: 3,
                        sets: 3,
                        rpe: None,
                    },
                    forms::Exercise {
                        variant: forms::ExerciseVariant::Squat,
                        description: String::from("Competition"),
                        weight: 140.0,
                        reps: 3,
                        sets: 3,
                        rpe: Some(8.5),
                    },
                ],
            },
            DatedWorkout {
                recorded: second_day,
                exercises: vec![forms::Exercise {
                    variant: forms::ExerciseVariant::Deadlift,
                    description: String::from("Competition"),
                    weight: 160.0,
                    reps: 3,
                    sets: 3,
                    rpe: Some(7.5),
                }],
            },
        ];

        assert_eq!(grouped, expected);
    }
}
