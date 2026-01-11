import { useQuery } from "@tanstack/react-query";
import axios from "axios";
import { ExerciseVariant, LastExerciseSession } from "~/shared/types";

const fetchLastSession = async (params: {
  variant: ExerciseVariant;
  description: string;
  currentDate: string;
}): Promise<LastExerciseSession | null> => {
  const response = await axios.post<LastExerciseSession | null>(
    `/exercises/last-session`,
    {
      variant: params.variant,
      description: params.description,
      currentDate: params.currentDate,
    },
  );
  return response.data;
};

export const useLastSession = (
  variant?: ExerciseVariant,
  description?: string,
  currentDate?: string,
) => {
  return useQuery(
    ["lastSession", variant, description, currentDate],
    () =>
      fetchLastSession({
        variant: variant!,
        description: description!,
        currentDate: currentDate!,
      }),
    {
      enabled:
        !!variant &&
        variant !== ExerciseVariant.Unknown &&
        !!description &&
        description.trim() !== "" &&
        !!currentDate,
    },
  );
};
