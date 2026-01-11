import { createSlice, PayloadAction } from "@reduxjs/toolkit";

interface UserState {
  token?: string;
}

const initialState: UserState = {
  token: undefined,
};

export const userSlice = createSlice({
  name: "user",
  initialState,
  reducers: {
    logout: (state) => {
      state.token = undefined;
    },
    setToken: (state, action: PayloadAction<string>) => {
      state.token = action.payload;
    },
  },
});

export const { logout, setToken } = userSlice.actions;

export default userSlice.reducer;
