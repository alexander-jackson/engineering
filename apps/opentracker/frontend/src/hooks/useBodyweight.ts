import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { DateTime } from "luxon";
import {
  fetchAllBodyweights,
  fetchBodyweightByDate,
  updateBodyweight,
  deleteBodyweight,
  BodyweightResponse,
} from "~/api/bodyweight";

enum Operation {
  Insert,
  Replace,
}

// Preserve the optimistic update logic from bodyweightSlice
const findInsertionIndex = (
  label: string,
  labels: string[],
): { index: number; operation: Operation } => {
  const left = DateTime.fromISO(label);

  for (let i = 0; i < labels.length; i++) {
    const right = DateTime.fromISO(labels[i]);

    if (left < right) {
      return { index: i, operation: Operation.Insert };
    }

    if (left.equals(right)) {
      return { index: i, operation: Operation.Replace };
    }
  }

  return { index: labels.length, operation: Operation.Insert };
};

export const useBodyweights = () => {
  return useQuery(["bodyweights"], fetchAllBodyweights);
};

export const useBodyweightByDate = (recorded: string) => {
  return useQuery(
    ["bodyweight", recorded],
    () => fetchBodyweightByDate(recorded),
    {
      enabled: !!recorded,
    },
  );
};

export const useUpdateBodyweight = () => {
  const queryClient = useQueryClient();

  return useMutation(updateBodyweight, {
    onMutate: async (params) => {
      // Cancel in-flight queries
      await queryClient.cancelQueries(["bodyweights"]);

      // Save previous state
      const previous = queryClient.getQueryData<BodyweightResponse>([
        "bodyweights",
      ]);

      // Optimistically update cache
      if (previous) {
        const newLabels = [...previous.labels];
        const newValues = [...previous.values];
        const value = params.bodyweight;

        const { index, operation } = findInsertionIndex(
          params.recorded,
          newLabels,
        );

        switch (operation) {
          case Operation.Replace:
            newValues[index] = value;
            break;
          case Operation.Insert:
            newLabels.splice(index, 0, params.recorded);
            newValues.splice(index, 0, value);
            break;
        }

        queryClient.setQueryData<BodyweightResponse>(["bodyweights"], {
          labels: newLabels,
          values: newValues,
        });
      }

      return { previous };
    },
    onError: (err, variables, context) => {
      // Rollback on error
      if (context?.previous) {
        queryClient.setQueryData(["bodyweights"], context.previous);
      }
    },
    onSettled: () => {
      // Refetch after mutation
      queryClient.invalidateQueries(["bodyweights"]);
    },
  });
};

export const useDeleteBodyweight = () => {
  const queryClient = useQueryClient();

  return useMutation(deleteBodyweight, {
    onMutate: async (recorded) => {
      // Cancel in-flight queries
      await queryClient.cancelQueries(["bodyweights"]);

      // Save previous state
      const previous = queryClient.getQueryData<BodyweightResponse>([
        "bodyweights",
      ]);

      // Optimistically update cache
      if (previous) {
        const index = previous.labels.indexOf(recorded);

        if (index !== -1) {
          const newLabels = [...previous.labels];
          const newValues = [...previous.values];

          newLabels.splice(index, 1);
          newValues.splice(index, 1);

          queryClient.setQueryData<BodyweightResponse>(["bodyweights"], {
            labels: newLabels,
            values: newValues,
          });
        }
      }

      return { previous };
    },
    onError: (err, variables, context) => {
      // Rollback on error
      if (context?.previous) {
        queryClient.setQueryData(["bodyweights"], context.previous);
      }
    },
    onSettled: () => {
      // Refetch after mutation
      queryClient.invalidateQueries(["bodyweights"]);
    },
  });
};
