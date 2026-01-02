import { screen, waitFor } from "@testing-library/react";

import render from "../utils";
import PercentageChange from "~/components/PercentageChange";

test.each([
  [100, 50, "(-50%)", "red"],
  [100, 150, "(+50%)", "green"],
  [100, 100, "(no change)", ""],
])(
  "percentages are shown correctly for %s => %s",
  async (prior, current, text, colour) => {
    // Render the component itself
    render(<PercentageChange prior={prior} current={current} />);

    await waitFor(() => expect(screen.getByText(text)).toBeDefined());

    const element = screen.getByText(text);

    expect(element.style.color).toBe(colour);
  },
);
