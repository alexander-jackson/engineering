use std::fmt;

use sqlx::Type;

#[derive(
    Copy, Clone, Debug, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize, Deserialize, Type,
)]
#[sqlx(transparent)]
pub struct RecordedDate(pub chrono::NaiveDate);

impl From<chrono::NaiveDate> for RecordedDate {
    fn from(value: chrono::NaiveDate) -> Self {
        Self(value)
    }
}

#[derive(Eq, PartialEq, Deserialize)]
pub struct Password(String);

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<redacted>")
    }
}

impl AsRef<[u8]> for Password {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(Debug, Deserialize)]
pub struct Registration {
    pub email: String,
    pub password: Password,
}

#[derive(Debug, Deserialize)]
pub struct Login {
    pub email: String,
    pub password: Password,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePassword {
    pub current_password: Password,
    pub new_password: Password,
    pub repeat_password: Password,
}

#[derive(Debug, Deserialize)]
pub struct Bodyweight {
    pub bodyweight: f32,
}

#[derive(Debug, Deserialize)]
pub struct Workout {
    pub exercises: Vec<Exercise>,
}

#[derive(sqlx::Type)]
#[sqlx(type_name = "exercise_variant")]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ExerciseVariant {
    Squat,
    Bench,
    Deadlift,
    Other,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Exercise {
    pub variant: ExerciseVariant,
    pub description: String,
    pub weight: f32,
    pub reps: i32,
    pub sets: i32,
    pub rpe: Option<f32>,
}

impl From<crate::persistence::workouts::DatedExercise> for Exercise {
    fn from(v: crate::persistence::workouts::DatedExercise) -> Self {
        Exercise {
            variant: v.variant,
            description: v.description,
            weight: v.weight,
            reps: v.reps,
            sets: v.sets,
            rpe: v.rpe,
        }
    }
}
