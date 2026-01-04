import configureStore from "redux-mock-store";
import thunk from "redux-thunk";
import axios from "axios";
import MockAdapter from "axios-mock-adapter";

import {
  pendingExerciseSlice,
  clearLastSession,
  fetchLastSession,
  PendingExerciseState,
} from "./pendingExerciseSlice";
import { ExerciseVariant } from "~/shared/types";

const middlewares = [thunk];
const mockStore = configureStore(middlewares);
const mock = new MockAdapter(axios);

describe("pendingExerciseSlice", () => {
  afterEach(() => {
    mock.reset();
  });

  describe("clearLastSession reducer", () => {
    it("clears last session and loading state", () => {
      const initialState: PendingExerciseState = {
        variant: ExerciseVariant.Squat,
        description: "Competition",
        lastSession: {
          recorded: "2024-01-15",
          exercise: {
            variant: ExerciseVariant.Squat,
            description: "Competition",
            weight: 100,
            reps: 5,
            sets: 3,
            rpe: 7.5,
          },
        },
        lastSessionLoading: false,
      };

      const nextState = pendingExerciseSlice.reducer(
        initialState,
        clearLastSession(),
      );

      expect(nextState.lastSession).toBeNull();
      expect(nextState.lastSessionLoading).toBe(false);
      // Other fields should remain unchanged
      expect(nextState.variant).toBe(ExerciseVariant.Squat);
      expect(nextState.description).toBe("Competition");
    });
  });

  describe("fetchLastSession async thunk", () => {
    it("sets loading state to true when pending", () => {
      const initialState: PendingExerciseState = {
        lastSessionLoading: false,
      };

      const action = { type: fetchLastSession.pending.type };
      const nextState = pendingExerciseSlice.reducer(initialState, action);

      expect(nextState.lastSessionLoading).toBe(true);
    });

    it("sets last session data and loading false when fulfilled", () => {
      const initialState: PendingExerciseState = {
        lastSessionLoading: true,
      };

      const lastSessionData = {
        recorded: "2024-01-15",
        exercise: {
          variant: ExerciseVariant.Squat,
          description: "Competition",
          weight: 100,
          reps: 5,
          sets: 3,
          rpe: 7.5,
        },
      };

      const action = {
        type: fetchLastSession.fulfilled.type,
        payload: lastSessionData,
      };

      const nextState = pendingExerciseSlice.reducer(initialState, action);

      expect(nextState.lastSession).toEqual(lastSessionData);
      expect(nextState.lastSessionLoading).toBe(false);
    });

    it("clears last session and sets loading false when rejected", () => {
      const initialState: PendingExerciseState = {
        lastSessionLoading: true,
        lastSession: {
          recorded: "2024-01-15",
          exercise: {
            variant: ExerciseVariant.Squat,
            description: "Competition",
            weight: 100,
            reps: 5,
            sets: 3,
            rpe: 7.5,
          },
        },
      };

      const action = { type: fetchLastSession.rejected.type };
      const nextState = pendingExerciseSlice.reducer(initialState, action);

      expect(nextState.lastSession).toBeNull();
      expect(nextState.lastSessionLoading).toBe(false);
    });

    it("makes correct API call with variant, description, and currentDate", async () => {
      const lastSessionData = {
        recorded: "2024-01-15",
        exercise: {
          variant: ExerciseVariant.Squat,
          description: "Competition",
          weight: 100,
          reps: 5,
          sets: 3,
          rpe: 7.5,
        },
      };

      mock
        .onPost("/exercises/last-session", {
          variant: ExerciseVariant.Squat,
          description: "Competition",
          currentDate: "2024-01-20",
        })
        .reply(200, lastSessionData);

      const store = mockStore({});

      await store.dispatch(
        fetchLastSession({
          variant: ExerciseVariant.Squat,
          description: "Competition",
          currentDate: "2024-01-20",
        }) as any,
      );

      const actions = store.getActions();
      expect(actions[0].type).toBe(fetchLastSession.pending.type);
      expect(actions[1].type).toBe(fetchLastSession.fulfilled.type);
      expect(actions[1].payload).toEqual(lastSessionData);
    });

    it("handles null response when no previous session exists", async () => {
      mock
        .onPost("/exercises/last-session", {
          variant: ExerciseVariant.Bench,
          description: "Competition",
          currentDate: "2024-01-20",
        })
        .reply(200, null);

      const store = mockStore({});

      await store.dispatch(
        fetchLastSession({
          variant: ExerciseVariant.Bench,
          description: "Competition",
          currentDate: "2024-01-20",
        }) as any,
      );

      const actions = store.getActions();
      expect(actions[1].type).toBe(fetchLastSession.fulfilled.type);
      expect(actions[1].payload).toBeNull();
    });
  });
});
