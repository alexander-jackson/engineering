import { screen, fireEvent, waitFor } from "@testing-library/react";

import render from "./utils";
import Dashboard from "~/views/Dashboard";

test("competitions page is not shown", async () => {
  // Render the component itself
  render(<Dashboard />, { routerSettings: { initialEntries: ["/dashboard"] } });

  // Make sure we can't see the competitions page
  expect(screen.queryByText("Competitions")).toBeNull();
});
