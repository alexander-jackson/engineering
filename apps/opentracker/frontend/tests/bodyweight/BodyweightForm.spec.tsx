import { rest } from "msw";
import { setupServer } from "msw/node";
import { screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DateTime } from "luxon";

import render, { endpoint } from "../utils";
import BodyweightForm from "~/views/bodyweight/BodyweightForm";

const RECORDED_DATE_LABEL = "Recorded Date";
const BODYWEIGHT_LABEL = "Bodyweight";
const SELECTED_DATE = "2022-01-11";
const SELECTED_BODYWEIGHT = 83.4;
const CURRENT_DATE = DateTime.now().toISODate();
const CURRENT_BODYWEIGHT = 82.5;
const CHANGED_BODYWEIGHT = 81.4;

const server = setupServer(
  rest.get(endpoint(`/bodyweights/${CURRENT_DATE}`), (req, res, ctx) => {
    return res(ctx.json({ bodyweight: CURRENT_BODYWEIGHT }));
  }),
  rest.get(endpoint(`/bodyweights/${SELECTED_DATE}`), (req, res, ctx) => {
    return res(ctx.json({ bodyweight: SELECTED_BODYWEIGHT }));
  }),
  rest.put(endpoint(`/bodyweights/${CURRENT_DATE}`), (req, res, ctx) => {
    return res(ctx.status(200));
  }),
  rest.put(endpoint(`/bodyweights/${SELECTED_DATE}`), (req, res, ctx) => {
    return res(ctx.status(200));
  }),
  rest.delete(endpoint(`/bodyweights/${CURRENT_DATE}`), (req, res, ctx) => {
    return res(ctx.status(200));
  }),
);

beforeAll(() => server.listen());
afterAll(() => server.close());

test("initially uses the current date", () => {
  render(<BodyweightForm />);

  const recordedInput = screen.getByLabelText(RECORDED_DATE_LABEL);
  expect(recordedInput.value).toBe(CURRENT_DATE);
});

test("form is prefilled by the current date value", async () => {
  render(<BodyweightForm />);

  await waitFor(() => {
    const bodyweightInput = screen.getByLabelText(BODYWEIGHT_LABEL);
    expect(bodyweightInput.value).toBe(CURRENT_BODYWEIGHT.toString());
  });
});

test("users can submit the initial form", async () => {
  render(<BodyweightForm />);

  // Wait for the initial value to load
  await waitFor(() => {
    const bodyweightInput = screen.getByLabelText(BODYWEIGHT_LABEL);
    expect(bodyweightInput.value).toBe(CURRENT_BODYWEIGHT.toString());
  });

  // Try and submit the form
  fireEvent.click(screen.getByText("Submit"));

  // Ensure no error is shown (success)
  await waitFor(() => {
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

test("users can change the date", async () => {
  render(<BodyweightForm />);

  // Find the recorded field and change it
  const inputField = screen.getByLabelText(RECORDED_DATE_LABEL);
  userEvent.type(inputField, SELECTED_DATE);

  // Expect the value to be correct
  expect(inputField.value).toBe(SELECTED_DATE);
});

test("changing the date loads the bodyweight for that day", async () => {
  render(<BodyweightForm />);

  // Find the recorded field and change it
  const inputField = screen.getByLabelText(RECORDED_DATE_LABEL);
  userEvent.type(inputField, SELECTED_DATE);

  // Check that the loaded bodyweight is correct
  await waitFor(() =>
    expect(screen.getByLabelText(BODYWEIGHT_LABEL).value).toBe(
      SELECTED_BODYWEIGHT.toString(),
    ),
  );
});

test("users can submit forms after changing the date", async () => {
  render(<BodyweightForm />);

  // Find the recorded field and change it
  const inputField = screen.getByLabelText(RECORDED_DATE_LABEL);
  userEvent.type(inputField, SELECTED_DATE);

  // Check that the loaded bodyweight is correct
  await waitFor(() =>
    expect(screen.getByLabelText(BODYWEIGHT_LABEL).value).toBe(
      SELECTED_BODYWEIGHT.toString(),
    ),
  );

  // Try and submit the form
  fireEvent.click(screen.getByText("Submit"));

  // Ensure no error is shown (success)
  await waitFor(() => {
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

test("users can change their bodyweight", async () => {
  render(<BodyweightForm />);

  // Find the recorded field and change it
  const inputField = screen.getByLabelText(RECORDED_DATE_LABEL);
  userEvent.type(inputField, SELECTED_DATE);

  const bodyweightField = screen.getByLabelText(BODYWEIGHT_LABEL);

  // Check that the loaded bodyweight is correct
  await waitFor(() =>
    expect(bodyweightField.value).toBe(SELECTED_BODYWEIGHT.toString()),
  );

  // Change the bodyweight value
  userEvent.clear(bodyweightField);
  userEvent.type(bodyweightField, CHANGED_BODYWEIGHT.toString());

  // Try and submit the form
  fireEvent.click(screen.getByText("Submit"));

  // Ensure no error is shown (success)
  await waitFor(() => {
    expect(screen.queryByRole("alert")).toBeNull();
  });

  // Check the bodyweight value is correct
  await waitFor(() =>
    expect(bodyweightField.value).toBe(CHANGED_BODYWEIGHT.toString()),
  );
});

test("users can delete their bodyweight values", async () => {
  render(<BodyweightForm />);

  // Wait for the initial value to be rendered
  await waitFor(() => {
    const bodyweightInput = screen.getByLabelText(BODYWEIGHT_LABEL);
    expect(bodyweightInput.value).toBe(CURRENT_BODYWEIGHT.toString());
  });

  // Find the delete button and click it
  const deleteButton = screen.getByText("Delete");
  userEvent.click(deleteButton);

  // Ensure we removed the current value
  await waitFor(() =>
    expect(screen.getByLabelText(BODYWEIGHT_LABEL).value).toBe(""),
  );
});
