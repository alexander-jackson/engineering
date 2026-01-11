import { screen } from "@testing-library/react";

import render from "../utils";
import RepSetView from "~/components/RepSetView";
import { RepSetNotation } from "~/api/preferences";
import * as usePreferencesHook from "~/hooks/usePreferences";

test("component renders text in the expected order, reps then sets", () => {
  jest.spyOn(usePreferencesHook, "useUserPreferences").mockReturnValue({
    data: { repSetNotation: RepSetNotation.RepsThenSets },
  } as any);

  render(<RepSetView reps={6} sets={3} />);

  expect(screen.getByText("6x3")).toBeDefined();
});

test("component renders text in the expected order, sets then reps", () => {
  jest.spyOn(usePreferencesHook, "useUserPreferences").mockReturnValue({
    data: { repSetNotation: RepSetNotation.SetsThenReps },
  } as any);

  render(<RepSetView reps={6} sets={3} />);

  expect(screen.getByText("3x6")).toBeDefined();
});
