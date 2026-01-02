import axios from "axios";
import { createSlice, createAsyncThunk, PayloadAction } from "@reduxjs/toolkit";

import { RequestState } from "~/store/types";

interface UserState {
  token?: string;
  state?: RequestState;
}

const initialState: UserState = {
  token: undefined,
  state: undefined,
};

export const userSlice = createSlice({
  name: "user",
  initialState,
  reducers: {
    logout: (state) => {
      state.token = undefined;
    },
    resetUserState: (state) => {
      state.state = undefined;
    },
    setToken: (state, action: PayloadAction<string>) => {
      state.token = action.payload;
    },
  },
  extraReducers(builder) {
    builder.addCase(register.pending, (state, action) => {
      state.state = RequestState.Pending;
    });

    builder.addCase(register.fulfilled, (state, action) => {
      state.token = action.payload;
      state.state = RequestState.Persisted;
    });
  },
});

export const register = createAsyncThunk(
  "user/register",
  async ({ email, password }: { email: string; password: string }) => {
    const response = await axios.post<string>(`/register`, {
      email,
      password,
    });

    return response.data;
  },
);

export const { logout, resetUserState, setToken } = userSlice.actions;

export default userSlice.reducer;
