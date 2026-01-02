import { screen } from "@testing-library/react";

import render from "../utils";
import RepSetView from "~/components/RepSetView";
import userPreferencesReducer, {
  RepSetNotation,
} from "~/store/reducers/userPreferencesSlice";

test("component renders text in the expected order, reps then sets", () => {
  render(<RepSetView reps={6} sets={3} />, {
    preloadedState: {
      userPreferences: { repSetNotation: RepSetNotation.RepsThenSets },
    },
  });

  expect(screen.getByText("6x3")).toBeDefined();
});

test("component renders text in the expected order, sets then reps", () => {
  render(<RepSetView reps={6} sets={3} />, {
    preloadedState: {
      userPreferences: { repSetNotation: RepSetNotation.SetsThenReps },
    },
  });

  expect(screen.getByText("3x6")).toBeDefined();
});
