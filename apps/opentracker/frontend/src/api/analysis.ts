import axios from "axios";
import { ExerciseVariant } from "~/shared/types";

interface EstimatedMaxRecord {
  estimate: number;
  recorded: string;
}

export interface RepPersonalBest {
  weight: number;
  reps: number;
  recorded: string;
}

export interface ExerciseStatistics {
  estimatedMaxes: EstimatedMaxRecord[];
  repPersonalBests: RepPersonalBest[];
}

export const fetchUniqueExercises = async (
  variant: ExerciseVariant,
): Promise<string[]> => {
  const response = await axios.post<string[]>(`/exercises/unique`, {
    variant,
  });
  return response.data;
};

export const fetchExerciseStatistics = async (params: {
  variant: ExerciseVariant;
  description: string;
}): Promise<ExerciseStatistics> => {
  const response = await axios.post<ExerciseStatistics>(
    `/exercises/statistics`,
    { variant: params.variant, description: params.description },
  );
  return response.data;
};
