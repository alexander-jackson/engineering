import axios from "axios";
import { Exercise } from "~/shared/types";

export const fetchWorkout = async (recorded: string): Promise<Exercise[]> => {
  const response = await axios.get<Exercise[]>(`/workouts/${recorded}`);
  return response.data;
};

export const updateWorkout = async (params: {
  recorded: string;
  exercises: Exercise[];
}): Promise<void> => {
  await axios.put(`/workouts/${params.recorded}`, {
    exercises: params.exercises,
  });
};

export const deleteWorkout = async (recorded: string): Promise<void> => {
  await axios.delete(`/workouts/${recorded}`);
};
