import axios from "axios";
import { createSlice, createAsyncThunk, PayloadAction } from "@reduxjs/toolkit";

import { RootState } from "~/store";
import { RequestState } from "~/store/types";

export enum RepSetNotation {
  RepsThenSets = "RepsThenSets",
  SetsThenReps = "SetsThenReps",
}

interface UserPreferenceState {
  state?: RequestState;
  repSetNotation: RepSetNotation;
}

const initialState: UserPreferenceState = {
  state: undefined,
  repSetNotation: RepSetNotation.SetsThenReps,
};

export const userPreferencesSlice = createSlice({
  name: "userPreferences",
  initialState,
  reducers: {
    setRepSetNotation: (state, action: PayloadAction<RepSetNotation>) => {
      state.repSetNotation = action.payload;
    },
    resetRequestState: (state) => {
      state.state = undefined;
    },
  },
  extraReducers(builder) {
    builder.addCase(fetchUserPreferences.fulfilled, (state, action) => {
      if (action.payload !== null) {
        state.repSetNotation = action.payload.repSetNotation;
      }
    });

    builder.addCase(persistUserPreferences.pending, (state, action) => {
      state.state = RequestState.Pending;
    });

    builder.addCase(persistUserPreferences.fulfilled, (state, action) => {
      state.state = RequestState.Persisted;
    });
  },
});

export const fetchUserPreferences = createAsyncThunk(
  "userPreferences/fetchUserPreferences",
  async () => {
    const response = await axios.get(`/preferences`);

    return response.data;
  },
);

export const persistUserPreferences = createAsyncThunk<
  void,
  void,
  { state: RootState }
>("userPreferences/persistUserPreferences", async (_, thunkApi) => {
  const payload = thunkApi.getState().userPreferences;
  await axios.put(`/preferences`, payload);
});

export const { setRepSetNotation, resetRequestState } =
  userPreferencesSlice.actions;

export default userPreferencesSlice.reducer;
