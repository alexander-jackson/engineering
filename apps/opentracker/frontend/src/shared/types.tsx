export enum ExerciseVariant {
  Unknown = "Unknown",
  Squat = "Squat",
  Bench = "Bench",
  Deadlift = "Deadlift",
  Other = "Other",
}

export interface Exercise {
  variant: ExerciseVariant;
  description: string;
  weight: number;
  reps: number;
  sets: number;
  rpe?: number;
}

export interface DatedWorkout {
  recorded: string;
  exercises: Array<Exercise>;
}

export interface ExerciseDetails {
  weight: number;
  reps: number;
  sets: number;
  rpe?: number;
}

export interface GroupedExercise {
  variant: ExerciseVariant;
  description: string;
  groups: Array<ExerciseDetails>;
}
