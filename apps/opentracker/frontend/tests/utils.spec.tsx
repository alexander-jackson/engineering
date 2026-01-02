import { DateTime } from "luxon";

import { findLowerBoundIndex, findUpperBoundIndex } from "~/shared/utils";

const labels = [
  DateTime.fromISO("2022-01-01"),
  DateTime.fromISO("2022-01-05"),
  DateTime.fromISO("2022-01-10"),
  DateTime.fromISO("2022-02-02"),
  DateTime.fromISO("2022-02-15"),
];

test("lower bounds can be found by matches", () => {
  const lowerBound = labels[1];
  const index = findLowerBoundIndex(labels, lowerBound);

  expect(index).toBe(1);
});

test("lower bounds can be found by interpolation", () => {
  const lowerBound = DateTime.fromISO("2022-01-03");
  const index = findLowerBoundIndex(labels, lowerBound);

  expect(index).toBe(1);
});

test("lower bounds will default to undefined on no matches", () => {
  const lowerBound = DateTime.fromISO("2022-03-01");
  const index = findLowerBoundIndex(labels, lowerBound);

  expect(index).not.toBeDefined();
});

test("upper bounds can be found by matches", () => {
  const upperBound = labels[1];
  const index = findUpperBoundIndex(labels, upperBound);

  expect(index).toBe(2);
});

test("upper bounds can be found by interpolation", () => {
  const upperBound = DateTime.fromISO("2022-01-07");
  const index = findUpperBoundIndex(labels, upperBound);

  expect(index).toBe(2);
});

test("upper bounds will default to undefined on no matches", () => {
  const upperBound = DateTime.fromISO("2021-12-31");
  const index = findUpperBoundIndex(labels, upperBound);

  expect(index).not.toBeDefined();
});
