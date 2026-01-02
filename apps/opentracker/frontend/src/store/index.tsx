import { configureStore } from "@reduxjs/toolkit";

import userReducer from "~/store/reducers/userSlice";
import userPreferencesReducer from "~/store/reducers/userPreferencesSlice";
import bodyweightReducer from "~/store/reducers/bodyweightSlice";
import pendingExerciseReducer from "~/store/reducers/pendingExerciseSlice";
import workoutReducer from "~/store/reducers/workoutSlice";
import analysisReducer from "~/store/reducers/analysisSlice";

export const reducer = {
  user: userReducer,
  userPreferences: userPreferencesReducer,
  bodyweight: bodyweightReducer,
  pendingExercise: pendingExerciseReducer,
  workout: workoutReducer,
  analysis: analysisReducer,
};

const store = configureStore({ reducer });

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;

export default store;
