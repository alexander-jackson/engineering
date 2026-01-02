import { createSlice, PayloadAction } from "@reduxjs/toolkit";

import { Exercise, ExerciseVariant } from "~/shared/types";

export interface PendingExercise {
  variant?: ExerciseVariant;
  description?: string;
  weight?: number;
  reps?: number;
  sets?: number;
  rpe?: number;
}

export interface PendingExerciseState {
  variant?: ExerciseVariant;
  description?: string;
  weight?: number;
  reps?: number;
  sets?: number;
  rpe?: number;
  index?: number;
}

const initialState: PendingExerciseState = {};

export const pendingExerciseSlice = createSlice({
  name: "pendingExercise",
  initialState,
  reducers: {
    setFromExercise: (
      state,
      action: PayloadAction<{ exercise: Exercise; index: number }>,
    ) => {
      const { variant, description, weight, reps, sets, rpe } =
        action.payload.exercise;

      state.variant = variant;
      state.description = description;
      state.weight = weight;
      state.reps = reps;
      state.sets = sets;
      state.rpe = rpe;

      state.index = action.payload.index;
    },
    setVariant: (state, action: PayloadAction<ExerciseVariant>) => {
      state.variant = action.payload;
    },
    setDescription: (state, action: PayloadAction<string>) => {
      state.description = action.payload;
    },
    setWeight: (state, action: PayloadAction<number>) => {
      state.weight = action.payload;
    },
    setReps: (state, action: PayloadAction<number>) => {
      state.reps = action.payload;
    },
    setSets: (state, action: PayloadAction<number>) => {
      state.sets = action.payload;
    },
    setRpe: (state, action: PayloadAction<number>) => {
      state.rpe = action.payload;
    },
    reset: (state) => {
      state.variant = ExerciseVariant.Unknown;
      state.description = "";
      state.weight = undefined;
      state.reps = undefined;
      state.sets = undefined;
      state.rpe = undefined;
      state.index = undefined;
    },
  },
});

export const {
  setFromExercise,
  setVariant,
  setDescription,
  setWeight,
  setReps,
  setSets,
  setRpe,
  reset,
} = pendingExerciseSlice.actions;

export default pendingExerciseSlice.reducer;
