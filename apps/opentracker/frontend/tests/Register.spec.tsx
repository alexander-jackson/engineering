import { rest } from "msw";
import { setupServer } from "msw/node";
import { screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import render, { endpoint } from "./utils";
import Register from "~/views/Register";

const EMAIL = "some@email.com";
const PASSWORD = "some-password";

const server = setupServer(
  rest.post(endpoint("/register"), (req, res, ctx) => {
    return res(ctx.json("something"));
  }),
  rest.get(endpoint("/email/status"), (req, res, ctx) => {
    return res(ctx.json({ email: "example@address.com", verified: true }));
  }),
);

beforeAll(() => server.listen());
afterAll(() => server.close());

test("users can register", async () => {
  // Render the component itself
  render(<Register />, { routerSettings: { initialEntries: ["/register"] } });

  const emailInput = screen.getByLabelText("Email address");
  const passwordInput = screen.getByLabelText("Password");

  userEvent.type(emailInput, EMAIL);
  userEvent.type(passwordInput, PASSWORD);

  // Click the submit button
  userEvent.click(screen.getAllByText("Register").at(1));

  // Wait until we can see the dashboard
  await waitFor(() => expect(screen.getByText("Dashboard")).toBeDefined());
});

test("failed registrations do not show the dashboard", async () => {
  server.use(
    rest.post(endpoint("/register"), (req, res, ctx) => {
      return res(ctx.status(401));
    }),
  );

  // Render the component itself
  render(<Register />, { routerSettings: { initialEntries: ["/register"] } });

  const emailInput = screen.getByLabelText("Email address");
  const passwordInput = screen.getByLabelText("Password");

  userEvent.type(emailInput, EMAIL);
  userEvent.type(passwordInput, PASSWORD);

  // Click the submit button
  userEvent.click(screen.getAllByText("Register").at(1));

  await expect(
    waitFor(() => screen.getByText("Dashboard")),
  ).rejects.toBeDefined();
});
