import axios from "axios";
import { createSlice, createAsyncThunk, PayloadAction } from "@reduxjs/toolkit";
import { DateTime } from "luxon";

import { RequestState } from "~/store/types";

interface BodyweightStateResponse {
  labels: Array<string>;
  values: Array<number>;
}

interface BodyweightState {
  labels: Array<string>;
  values: Array<string>;
  state?: RequestState;
}

const initialState: BodyweightState = {
  labels: [],
  values: [],
  state: undefined,
};

enum Operation {
  Insert,
  Replace,
}

const findInsertionIndex = (
  label: string,
  labels: Array<string>,
): { index: number; operation: Operation } => {
  const left = DateTime.fromISO(label);

  for (let i = 0; i < labels.length; i++) {
    const right = DateTime.fromISO(labels[i]);

    if (left < right) {
      return { index: i, operation: Operation.Insert };
    }

    if (left.equals(right)) {
      return { index: i, operation: Operation.Replace };
    }
  }

  return { index: labels.length, operation: Operation.Insert };
};

export const bodyweightSlice = createSlice({
  name: "bodyweight",
  initialState,
  reducers: {
    fetchAll: (state) => {
      axios.get<BodyweightStateResponse>(`/bodyweights`).then((response) => {
        state.labels = response.data.labels;
        state.values = response.data.values.map((x) => x.toFixed(2));
      });
    },
    set: (state, action: PayloadAction<BodyweightState>) => {
      state.labels = action.payload.labels;
      state.values = action.payload.values;
    },
    reset: (state) => {
      state.labels = [];
      state.values = [];
    },
    resetRequestState: (state) => {
      state.state = undefined;
    },
  },
  extraReducers(builder) {
    builder.addCase(fetchAllBodyweightEntries.fulfilled, (state, action) => {
      state.labels = action.payload.labels;
      state.values = action.payload.values.map((x) => x.toFixed(2));
    });

    builder.addCase(putBodyweightEntry.pending, (state) => {
      state.state = RequestState.Pending;
    });

    builder.addCase(putBodyweightEntry.fulfilled, (state, action) => {
      state.state = RequestState.Persisted;
      const value = action.payload.bodyweight.toFixed(2);

      // Find the index to insert
      const { index, operation } = findInsertionIndex(
        action.payload.recorded,
        state.labels,
      );

      switch (operation) {
        case Operation.Replace:
          state.values[index] = value;
          break;
        case Operation.Insert:
          state.labels.splice(index, 0, action.payload.recorded);
          state.values.splice(index, 0, value);
      }
    });

    builder.addCase(deleteBodyweightEntry.fulfilled, (state, action) => {
      // Find the index of the entry
      const index = state.labels.indexOf(action.payload.recorded);

      state.labels.splice(index, 1);
      state.values.splice(index, 1);
    });
  },
});

export const fetchAllBodyweightEntries = createAsyncThunk(
  "bodyweight/fetchAllBodyweightEntries",
  async () => {
    const response = await axios.get<BodyweightStateResponse>(`/bodyweights`);
    return response.data;
  },
);

export const putBodyweightEntry = createAsyncThunk(
  "bodyweight/putBodyweightEntry",
  async ({
    recorded,
    bodyweight,
  }: {
    recorded: string;
    bodyweight: number;
  }) => {
    await axios.put(`/bodyweights/${recorded}`, { bodyweight });

    return { recorded, bodyweight };
  },
);

export const deleteBodyweightEntry = createAsyncThunk(
  "bodyweight/deleteBodyweightEntry",
  async (recorded: string) => {
    await axios.delete(`/bodyweights/${recorded}`);

    return { recorded };
  },
);

export const { fetchAll, set, reset, resetRequestState } =
  bodyweightSlice.actions;

export default bodyweightSlice.reducer;
