import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchWorkout, updateWorkout, deleteWorkout } from "~/api/workouts";
import { Exercise } from "~/shared/types";

export const useWorkout = (recorded: string) => {
  return useQuery(["workout", recorded], () => fetchWorkout(recorded), {
    enabled: !!recorded,
    retry: false,
    onError: () => {
      // Return empty array on 404
      return { data: [] };
    },
  });
};

export const useUpdateWorkout = () => {
  const queryClient = useQueryClient();

  return useMutation(updateWorkout, {
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries(["workout", variables.recorded]);
      queryClient.invalidateQueries(["workouts"]); // Historical workouts
    },
  });
};

export const useDeleteWorkout = () => {
  const queryClient = useQueryClient();

  return useMutation(deleteWorkout, {
    onSuccess: (_, recorded) => {
      queryClient.invalidateQueries(["workout", recorded]);
      queryClient.invalidateQueries(["workouts"]); // Historical workouts
    },
  });
};
