import { DateTime } from "luxon";

import { Exercise, ExerciseVariant, GroupedExercise } from "~/shared/types";

export const findLowerBoundIndex = (
  labels: Array<DateTime>,
  lowerBound: DateTime,
): number | undefined => {
  for (let i = 0; i < labels.length; i++) {
    if (lowerBound <= labels[i]) {
      return i;
    }
  }

  return undefined;
};

export const findUpperBoundIndex = (
  labels: Array<DateTime>,
  upperBound: DateTime,
): number | undefined => {
  // Check the initial index and return undefined
  if (upperBound < labels[0]) {
    return undefined;
  }

  for (let i = 1; i < labels.length; i++) {
    const value = labels[i];

    if (upperBound === value) {
      return i + 1;
    }

    if (upperBound < value) {
      return i;
    }
  }
};

export const groupByExercise = (
  exercises: Array<Exercise>,
): Array<GroupedExercise> => {
  const groupMap = exercises.reduce((map, current) => {
    const { variant, description, weight, reps, sets, rpe } = current;

    const key = JSON.stringify({ variant, description });
    const value = { weight, reps, sets, rpe };

    if (!map.has(key)) {
      map.set(key, []);
    }

    map.get(key).push(value);

    return map;
  }, new Map());

  return Array.from(groupMap, ([serializedKey, value]) => {
    const { variant, description } = JSON.parse(serializedKey) as {
      variant: ExerciseVariant;
      description: string;
    };

    return { variant, description, groups: value };
  });
};
