import axios from "axios";

export interface BodyweightResponse {
  labels: string[];
  values: number[];
}

export interface BodyweightEntry {
  bodyweight: number;
}

export const fetchAllBodyweights = async (): Promise<BodyweightResponse> => {
  const response = await axios.get<BodyweightResponse>(`/bodyweights`);
  return response.data;
};

export const fetchBodyweightByDate = async (
  recorded: string,
): Promise<BodyweightEntry> => {
  const response = await axios.get<BodyweightEntry>(`/bodyweights/${recorded}`);
  return response.data;
};

export const updateBodyweight = async (params: {
  recorded: string;
  bodyweight: number;
}): Promise<void> => {
  await axios.put(`/bodyweights/${params.recorded}`, {
    bodyweight: params.bodyweight,
  });
};

export const deleteBodyweight = async (recorded: string): Promise<void> => {
  await axios.delete(`/bodyweights/${recorded}`);
};
