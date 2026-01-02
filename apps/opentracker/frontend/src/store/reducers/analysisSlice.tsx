import axios from "axios";
import { createSlice, createAsyncThunk, PayloadAction } from "@reduxjs/toolkit";

import { ExerciseVariant } from "~/shared/types";

interface EstimatedMaxRecord {
  estimate: number;
  recorded: string;
}

export interface RepPersonalBest {
  weight: number;
  reps: number;
  recorded: string;
}

export interface ExerciseStatistics {
  estimatedMaxes: Array<EstimatedMaxRecord>;
  repPersonalBests: Array<RepPersonalBest>;
}

interface AnalysisState {
  variant: ExerciseVariant;
  description: string;
  uniqueExercises: Array<string>;
  exerciseStatistics?: ExerciseStatistics;
}

const initialState: AnalysisState = {
  variant: ExerciseVariant.Unknown,
  description: "",
  uniqueExercises: [],
};

export const analysisSlice = createSlice({
  name: "analysis",
  initialState,
  reducers: {
    setVariant: (state, action: PayloadAction<ExerciseVariant>) => {
      state.variant = action.payload;
    },
    setDescription: (state, action: PayloadAction<string>) => {
      state.description = action.payload;
    },
  },
  extraReducers(builder) {
    builder.addCase(fetchUniqueExercises.fulfilled, (state, action) => {
      state.uniqueExercises = action.payload;
      state.description = action.payload[0] || "";
    });

    builder.addCase(fetchExerciseStatistics.fulfilled, (state, action) => {
      state.exerciseStatistics = action.payload;
    });
  },
});

export const fetchUniqueExercises = createAsyncThunk(
  "analysis/fetchUniqueExercises",
  async (variant: ExerciseVariant) => {
    const response = await axios.post<Array<string>>(`/exercises/unique`, {
      variant,
    });

    return response.data;
  },
);

export const fetchExerciseStatistics = createAsyncThunk(
  "analysis/fetchExerciseStatistics",
  async ({
    variant,
    description,
  }: {
    variant: ExerciseVariant;
    description: string;
  }) => {
    const response = await axios.post<ExerciseStatistics>(
      `/exercises/statistics`,
      { variant, description },
    );

    return response.data;
  },
);

export const { setVariant, setDescription } = analysisSlice.actions;

export default analysisSlice.reducer;
