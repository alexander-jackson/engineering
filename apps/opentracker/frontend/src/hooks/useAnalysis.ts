import { useQuery } from "@tanstack/react-query";
import { ExerciseVariant } from "~/shared/types";
import { fetchUniqueExercises, fetchExerciseStatistics } from "~/api/analysis";

export const useUniqueExercises = (variant: ExerciseVariant) => {
  return useQuery(
    ["uniqueExercises", variant],
    () => fetchUniqueExercises(variant),
    {
      enabled: variant !== ExerciseVariant.Unknown,
    },
  );
};

export const useExerciseStatistics = (
  variant: ExerciseVariant,
  description: string,
) => {
  return useQuery(
    ["exerciseStatistics", variant, description],
    () => fetchExerciseStatistics({ variant, description }),
    {
      enabled:
        variant !== ExerciseVariant.Unknown &&
        description !== "" &&
        description.trim() !== "",
    },
  );
};
