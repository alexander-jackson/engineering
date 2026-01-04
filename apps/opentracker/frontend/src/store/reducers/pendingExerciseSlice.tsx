import axios from "axios";
import { createSlice, createAsyncThunk, PayloadAction } from "@reduxjs/toolkit";

import { Exercise, ExerciseVariant, LastExerciseSession } from "~/shared/types";

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
  lastSession?: LastExerciseSession | null;
  lastSessionLoading?: boolean;
}

const initialState: PendingExerciseState = {
  lastSessionLoading: false,
};

export const fetchLastSession = createAsyncThunk(
  "pendingExercise/fetchLastSession",
  async ({
    variant,
    description,
    currentDate,
  }: {
    variant: ExerciseVariant;
    description: string;
    currentDate: string;
  }) => {
    const response = await axios.post<LastExerciseSession | null>(
      `/exercises/last-session`,
      { variant, description, currentDate },
    );
    return response.data;
  },
);

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
      state.lastSession = null;
      state.lastSessionLoading = false;
    },
    clearLastSession: (state) => {
      state.lastSession = null;
      state.lastSessionLoading = false;
    },
  },
  extraReducers(builder) {
    builder.addCase(fetchLastSession.pending, (state) => {
      state.lastSessionLoading = true;
    });
    builder.addCase(fetchLastSession.fulfilled, (state, action) => {
      state.lastSession = action.payload;
      state.lastSessionLoading = false;
    });
    builder.addCase(fetchLastSession.rejected, (state) => {
      state.lastSession = null;
      state.lastSessionLoading = false;
    });
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
  clearLastSession,
} = pendingExerciseSlice.actions;

export default pendingExerciseSlice.reducer;
