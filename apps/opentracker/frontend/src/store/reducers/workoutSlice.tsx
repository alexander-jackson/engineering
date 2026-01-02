import axios from "axios";
import { createSlice, createAsyncThunk, PayloadAction } from "@reduxjs/toolkit";

import { RootState } from "~/store";
import { RequestState } from "~/store/types";
import { Exercise } from "~/shared/types";

export interface WorkoutState {
  state?: RequestState;
  exercises: Array<Exercise>;
  displayModal: boolean;
}

const initialState: WorkoutState = {
  state: undefined,
  exercises: [],
  displayModal: false,
};

export const workoutSlice = createSlice({
  name: "workout",
  initialState,
  reducers: {
    resetWorkoutState: (state) => {
      state.state = undefined;
    },
    showAddExerciseModal: (state) => {
      state.displayModal = true;
    },
    hideAddExerciseModal: (state) => {
      state.displayModal = false;
    },
    addExercise: (state, action: PayloadAction<Exercise>) => {
      state.exercises.push(action.payload);
    },
    editExercise: (
      state,
      action: PayloadAction<{ index: number; exercise: Exercise }>,
    ) => {
      const { index, exercise } = action.payload;
      state.exercises[index] = exercise;
    },
    deleteExercise: (state, action: PayloadAction<number>) => {
      state.exercises.splice(action.payload, 1);
    },
  },
  extraReducers(builder) {
    builder.addCase(fetchStructuredWorkout.fulfilled, (state, action) => {
      state.exercises = action.payload;
    });

    builder.addCase(fetchStructuredWorkout.rejected, (state, action) => {
      state.exercises = [];
    });

    builder.addCase(putStructuredWorkout.pending, (state, action) => {
      state.state = RequestState.Pending;
    });

    builder.addCase(putStructuredWorkout.fulfilled, (state, action) => {
      state.state = RequestState.Persisted;
    });

    builder.addCase(deleteStructuredWorkout.fulfilled, (state, action) => {
      state.exercises = [];
    });
  },
});

export const fetchStructuredWorkout = createAsyncThunk(
  "workout/fetchStructuredWorkout",
  async (recorded: string) => {
    let response = await axios.get<Array<Exercise>>(`/workouts/${recorded}`);

    return response.data;
  },
);

export const putStructuredWorkout = createAsyncThunk<
  void,
  string,
  { state: RootState }
>("workout/putStructuredWorkout", async (recorded, thunkApi) => {
  const state = thunkApi.getState();

  // Map the enums into the right state
  const exercises = state.workout.exercises;

  await axios.put(`/workouts/${recorded}`, { exercises });
});

export const deleteStructuredWorkout = createAsyncThunk(
  "workout/deleteStructuredWorkout",
  async (recorded: string) => {
    await axios.delete(`/workouts/${recorded}`);
  },
);

export const {
  resetWorkoutState,
  showAddExerciseModal,
  hideAddExerciseModal,
  addExercise,
  editExercise,
  deleteExercise,
} = workoutSlice.actions;

export default workoutSlice.reducer;
