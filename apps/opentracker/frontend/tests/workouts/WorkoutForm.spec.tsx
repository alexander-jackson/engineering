import { rest } from "msw";
import { setupServer } from "msw/node";
import {
  screen,
  fireEvent,
  waitFor,
  waitForElementToBeRemoved,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DateTime } from "luxon";

import render, { endpoint } from "../utils";
import WorkoutForm from "~/views/workouts/WorkoutForm";

const RECORDED_DATE_LABEL = "Date";
const SELECTED_DATE = "2022-01-11";
const DATE_WITH_NO_EXERCISES = "2022-01-13";
const CURRENT_DATE = DateTime.now().toISODate();

const EMAIL = "some@email.com";
const PASSWORD = "some-password";

const EXERCISES_FOR_SELECTED = [
  {
    variant: "Bench",
    description: "Competition",
    weight: 80,
    reps: 6,
    sets: 3,
    rpe: undefined,
  },
  {
    variant: "Deadlift",
    description: "Competition",
    weight: 120,
    reps: 3,
    sets: 4,
    rpe: 8.5,
  },
];

const EXERCISES_FOR_TODAY = [
  {
    variant: "Squat",
    description: "Competition",
    weight: 107.5,
    reps: 4,
    sets: 4,
    rpe: 7.5,
  },
];

const UNIQUE_EXERCISES = ["Competition", "Pause"];

const server = setupServer(
  rest.get(endpoint(`/workouts/${CURRENT_DATE}`), (req, res, ctx) => {
    return res(ctx.json(EXERCISES_FOR_TODAY));
  }),
  rest.get(endpoint(`/workouts/${SELECTED_DATE}`), (req, res, ctx) => {
    return res(ctx.json(EXERCISES_FOR_SELECTED));
  }),
  rest.get(endpoint(`/workouts/${DATE_WITH_NO_EXERCISES}`), (req, res, ctx) => {
    return res(ctx.status(404));
  }),
  rest.post(endpoint(`/exercises/unique`), (req, res, ctx) => {
    return res(ctx.json(UNIQUE_EXERCISES));
  }),
  rest.put(endpoint(`/workouts/${CURRENT_DATE}`), (req, res, ctx) => {
    return res(ctx.status(200));
  }),
  rest.put(endpoint(`/workouts/${SELECTED_DATE}`), (req, res, ctx) => {
    return res(ctx.status(200));
  }),
);

beforeAll(() => server.listen());
afterAll(() => server.close());

test("initially uses the current date", () => {
  render(<WorkoutForm />);

  const recordedInput = screen.getByLabelText(RECORDED_DATE_LABEL);
  expect(recordedInput.value).toBe(CURRENT_DATE);
});

test("users can change the recorded date", () => {
  render(<WorkoutForm />);

  // Change the value by picking a date
  const recordedInput = screen.getByLabelText(RECORDED_DATE_LABEL);
  userEvent.type(recordedInput, SELECTED_DATE);

  expect(recordedInput.value).toBe(SELECTED_DATE);
});

test("changing the date updates the displayed exercises", async () => {
  render(<WorkoutForm />);

  // Change the date
  userEvent.type(screen.getByLabelText(RECORDED_DATE_LABEL), SELECTED_DATE);

  // Check we have the right state
  for (const exercise of EXERCISES_FOR_SELECTED) {
    await waitFor(() =>
      screen.getByAltText(`${exercise.variant} icon (black)`),
    );
  }
});

test("initial form renders exercises for today", async () => {
  render(<WorkoutForm />);

  // Check we have the right state
  for (const exercise of EXERCISES_FOR_TODAY) {
    await waitFor(() =>
      screen.getByAltText(`${exercise.variant} icon (black)`),
    );
  }
});

test("initial form displays a button to add exercises", async () => {
  render(<WorkoutForm />);

  // Check we have the button to add an exercise
  const button = screen.getByRole("button", { name: "add-exercise" });

  expect(button).toBeDefined();
});

test("initial form displays a button to delete workouts", async () => {
  render(<WorkoutForm />);

  // Check we have the button to add an exercise
  const button = screen.getByRole("button", { name: "delete-workout" });

  expect(button).toBeDefined();
});

test("initial form can be used to submit a workout", async () => {
  render(<WorkoutForm />);

  // Bring up the exercise modal
  const button = screen.getByRole("button", { name: "add-exercise" });
  fireEvent.click(button);

  await waitFor(() => screen.getByText("Add Exercise"));

  // Click the `Bench` logo
  userEvent.click(screen.getByAltText("Bench icon (white)"));

  // Fill out the exercise form
  const mappings = [
    { label: "Variation", value: "Competition" },
    { label: "Weight (kg)", value: "125" },
    { label: "Reps", value: "1" },
    { label: "Sets", value: "1" },
    { label: "RPE", value: "8.5" },
  ];

  mappings.forEach(({ label, value }) => {
    userEvent.type(screen.getByLabelText(label), value);
  });

  // Submit the modal
  fireEvent.click(screen.getByText("Submit"));

  // Wait for the modal to close
  await waitFor(() => expect(screen.queryByText("Add Exercise")).toBeNull());
});

test("date can be changed and then workouts can be uploaded", async () => {
  render(<WorkoutForm />);

  // Change the date
  const recordedInput = screen.getByLabelText(RECORDED_DATE_LABEL);
  userEvent.type(recordedInput, SELECTED_DATE);

  // Check the workout has loaded
  for (const exercise of EXERCISES_FOR_SELECTED) {
    await waitFor(() =>
      screen.getByAltText(`${exercise.variant} icon (black)`),
    );
  }

  // Bring up the exercise modal
  const button = screen.getByRole("button", { name: "add-exercise" });
  fireEvent.click(button);

  // Ensure it has opened correctly
  await waitFor(() => screen.getByText("Add Exercise"));

  // Click the `Bench` logo
  userEvent.click(screen.getByAltText("Bench icon (white)"));

  // Fill out the exercise form
  const mappings = [
    { label: "Variation", value: "Competition" },
    { label: "Weight (kg)", value: "125" },
    { label: "Reps", value: "1" },
    { label: "Sets", value: "1" },
    { label: "RPE", value: "8.5" },
  ];

  mappings.forEach(({ label, value }) => {
    userEvent.type(screen.getByLabelText(label), value);
  });

  // Submit the modal
  fireEvent.click(screen.getByRole("button", { name: "modal-submit" }));

  // Wait for the modal to close
  await waitFor(() => expect(screen.queryByText("Add Exercise")).toBeNull());
});

test("deleting the current workout prompts for confirmation", async () => {
  render(<WorkoutForm />);

  // Try and delete the workout
  const button = screen.getByRole("button", { name: "delete-workout" });
  userEvent.click(button);

  // Check we get a confirmation first
  await waitFor(() => screen.getByText("Delete Workout"));
});

test("cancelling the delete workout confirmation does not delete exercises", async () => {
  render(<WorkoutForm />);

  // Try and delete the workout
  const button = screen.getByRole("button", { name: "delete-workout" });
  userEvent.click(button);

  // Get the cancellation button (wait for modal to appear)
  const cancel = await waitFor(() =>
    screen.getByRole("button", { name: "cancel-pending-action" })
  );
  userEvent.click(cancel);

  // Validate all the exercises are still here
  for (const exercise of EXERCISES_FOR_TODAY) {
    await waitFor(() =>
      screen.getByAltText(`${exercise.variant} icon (black)`),
    );
  }
});
