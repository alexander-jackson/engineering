import { rest } from "msw";
import { setupServer } from "msw/node";
import { screen, waitFor } from "@testing-library/react";

import render, { endpoint } from "../utils";
import WeeklyVolumeStatistics from "~/components/WeeklyVolumeStatistics";

const server = setupServer(
  rest.get(endpoint("/workouts/statistics"), (req, res, ctx) => {
    return res(
      ctx.json({
        squatVolumePastWeek: 1000,
        benchVolumePastWeek: 500,
        deadliftVolumePastWeek: 2000,
        otherVolumePastWeek: 250,
      }),
    );
  }),
);

beforeAll(() => server.listen());
afterAll(() => server.close());

describe("WeeklyVolumeStatistics (component)", () => {
  test("statistics are shown to the user", async () => {
    // Render the component itself
    render(<WeeklyVolumeStatistics />);

    await waitFor(() => expect(screen.getByText("1000kg")).toBeDefined());
    await waitFor(() => expect(screen.getByText("500kg")).toBeDefined());
    await waitFor(() => expect(screen.getByText("2000kg")).toBeDefined());
    await waitFor(() => expect(screen.getByText("250kg")).toBeDefined());
  });

  test("should render nothing if no statistics for the last week", async () => {
    server.use(
      rest.get(endpoint("/workouts/statistics"), (req, res, ctx) => {
        return res(
          ctx.json({
            squatVolumePastWeek: null,
            benchVolumePastWeek: null,
            deadliftVolumePastWeek: null,
            otherVolumePastWeek: null,
          }),
        );
      }),
    );

    render(<WeeklyVolumeStatistics />);

    expect(screen.queryByText("Weekly Volume")).toBeNull();
  });

  test("should not show boxes for exercises with no statistics", async () => {
    server.use(
      rest.get(endpoint("/workouts/statistics"), (req, res, ctx) => {
        return res(
          ctx.json({
            squatVolumePastWeek: 2000,
            benchVolumePastWeek: 1000,
            deadliftVolumePastWeek: null,
            otherVolumePastWeek: null,
          }),
        );
      }),
    );

    render(<WeeklyVolumeStatistics />);

    await waitFor(() => expect(screen.getByText("Squat")).toBeDefined());
    expect(screen.queryByText("Deadlift")).toBeNull();
  });
});
