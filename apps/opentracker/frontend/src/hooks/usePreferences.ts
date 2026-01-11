import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  fetchPreferences,
  updatePreferences,
  UserPreferences,
} from "~/api/preferences";

export const useUserPreferences = () => {
  return useQuery(["preferences"], fetchPreferences);
};

export const useUpdatePreferences = () => {
  const queryClient = useQueryClient();

  return useMutation(updatePreferences, {
    onSuccess: () => {
      queryClient.invalidateQueries(["preferences"]);
    },
  });
};
