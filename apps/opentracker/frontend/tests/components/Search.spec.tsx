import { screen } from "@testing-library/react";

import render from "../utils";
import Search from "~/components/Search";

const HAYSTACK = ["apple", "orange"];
const TITLE = "Suggestions";
const NO_RESULT_TITLE = "No Suggestions";

test("component renders all options if nothing specified", () => {
  render(<Search haystack={HAYSTACK} />);

  expect(screen.getByText(TITLE)).toBeDefined();

  HAYSTACK.forEach((item) => expect(screen.getByText(item)).toBeDefined());
});

test.each([
  ["", HAYSTACK],
  ["e", HAYSTACK],
  ["oran", ["orange"]],
  ["app", ["apple"]],
])("search displays the expected results for '%s'", (needle, expected) => {
  render(<Search haystack={HAYSTACK} needle={needle} />);

  expect(screen.getByText(TITLE)).toBeDefined();

  expected.forEach((item) => expect(screen.getByText(item)).toBeDefined());
});

test("nothing is shown on an exact match", () => {
  render(<Search haystack={HAYSTACK} needle="apple" />);

  expect(screen.queryByText(TITLE)).toBeNull();
});

test("different heading is shown if there are no matches", () => {
  render(<Search haystack={HAYSTACK} needle="kwargs" />);

  expect(screen.queryByText(TITLE)).toBeNull();
  expect(screen.getByText(NO_RESULT_TITLE)).toBeDefined();
});

test.each(["", "some search value"])(
  "empty haystack shows nothing for search of '%s'",
  (needle) => {
    render(<Search haystack={[]} needle={needle} />);

    expect(screen.queryByText(TITLE)).toBeNull();
    expect(screen.queryByText(NO_RESULT_TITLE)).toBeNull();

    HAYSTACK.forEach((item) => expect(screen.queryByText(item)).toBeNull());
  },
);
