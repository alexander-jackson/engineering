import { screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import render from "../utils";
import { addExercise, editExercise } from "~/store/reducers/workoutSlice";
import AddExerciseModal from "~/views/workouts/AddExerciseModal";

const someBench = (description, weight, reps, sets, rpe) => {
  return {
    variant: "Bench",
    description,
    weight,
    reps,
    sets,
    rpe,
  };
};

const someExercise = (variant, description, weight, reps, sets, rpe) => {
  return { variant, description, weight, reps, sets, rpe };
};

const somePendingExercise = someBench("Competition", 125, 1, 1, 8.5);

const someExercises = [
  somePendingExercise,
  someBench("Spoto", 95, 3, 6, undefined),
];

test("no placeholders for the first exercise", () => {
  render(<AddExerciseModal />, {
    preloadedState: {
      workout: {
        exercises: [],
        displayModal: true,
      },
    },
  });

  const keys = ["Variation", "Weight (kg)", "Reps", "Sets", "RPE"];

  keys.forEach((label) =>
    expect(screen.getByLabelText(label)).toHaveProperty("placeholder", ""),
  );
});

test.each(["Squat", "Bench", "Deadlift", "Other"])(
  "uses last exercise if not editing (%s)",
  (variant) => {
    const exercise = someExercise(variant, "Pause", 95, 3, 6, undefined);

    render(<AddExerciseModal />, {
      preloadedState: {
        workout: {
          exercises: [exercise],
          displayModal: true,
        },
      },
    });

    const mappings = [
      { label: variant !== "Other" ? "Variation" : "Exercise", value: "Pause" },
      { label: "Weight (kg)", value: "95" },
      { label: "Reps", value: "3" },
      { label: "Sets", value: "6" },
      { label: "RPE", value: "" },
    ];

    mappings.forEach(({ label, value }) =>
      expect(screen.getByLabelText(label)).toHaveProperty("placeholder", value),
    );

    // Validate that the exercise button is selected
    expect(screen.getByLabelText(`${variant} button (selected)`)).toBeDefined();
  },
);

test.each(["Squat", "Bench", "Deadlift", "Other"])(
  "editing an exercise fills the state, not the placeholders (%s)",
  (variant) => {
    const pending = someExercise(variant, "Pause", 95, 3, 6, undefined);

    render(<AddExerciseModal />, {
      preloadedState: {
        workout: {
          exercises: someExercises,
          displayModal: true,
        },
        pendingExercise: pending,
      },
    });

    const mappings = [
      { label: variant !== "Other" ? "Variation" : "Exercise", value: "Pause" },
      { label: "Weight (kg)", value: "95" },
      { label: "Reps", value: "3" },
      { label: "Sets", value: "6" },
      { label: "RPE", value: "" },
    ];

    mappings.forEach(({ label, value }) =>
      expect(screen.getByLabelText(label)).toHaveProperty("value", value),
    );

    // Validate that the exercise button is selected
    expect(screen.getByLabelText(`${variant} button (selected)`)).toBeDefined();
  },
);

test("submitting uses the values, then the placeholders", () => {
  const dispatch = jest.fn();

  render(<AddExerciseModal dispatch={dispatch} />, {
    preloadedState: {
      workout: {
        exercises: [someBench("Competition", 125, 1, 1, 8.5)],
        displayModal: true,
      },
      pendingExercise: {
        variant: "Bench",
        description: "Competition",
        weight: 120,
        rpe: 8,
      },
    },
  });

  fireEvent.click(screen.getByText("Submit"));

  const expectedPayload = {
    variant: "Bench",
    description: "Competition",
    weight: 120,
    reps: 1,
    sets: 1,
    rpe: 8,
  };

  expect(dispatch).toHaveBeenCalledWith(addExercise(expectedPayload));
});

test.each(["Squat", "Bench", "Deadlift", "Other"])(
  "exercises can be added (%s)",
  (variant) => {
    const dispatch = jest.fn();
    const pending = someExercise(variant, "Pause", 95, 3, 6, undefined);

    render(<AddExerciseModal dispatch={dispatch} />, {
      preloadedState: {
        workout: {
          exercises: [],
          displayModal: true,
        },
        pendingExercise: pending,
      },
    });

    fireEvent.click(screen.getByText("Submit"));

    expect(dispatch).toHaveBeenCalledWith(addExercise(pending));
  },
);

test("exercises can be edited", () => {
  const dispatch = jest.fn();

  const pending = {
    index: 0,
    ...somePendingExercise,
    description: "some-other-name",
  };

  render(<AddExerciseModal dispatch={dispatch} />, {
    preloadedState: {
      workout: {
        exercises: [somePendingExercise],
        displayModal: true,
      },
      pendingExercise: { index: 0, ...pending },
    },
  });

  fireEvent.click(screen.getByText("Submit"));

  const expected = {
    index: 0,
    exercise: {
      variant: "Bench",
      description: "some-other-name",
      weight: 125,
      reps: 1,
      sets: 1,
      rpe: 8.5,
    },
  };

  expect(dispatch).toHaveBeenCalledWith(editExercise(expected));
});
