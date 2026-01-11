import axios from "axios";

export enum RepSetNotation {
  RepsThenSets = "RepsThenSets",
  SetsThenReps = "SetsThenReps",
}

export interface UserPreferences {
  repSetNotation: RepSetNotation;
}

export const fetchPreferences = async (): Promise<UserPreferences> => {
  const response = await axios.get<UserPreferences>(`/preferences`);
  return response.data;
};

export const updatePreferences = async (
  preferences: UserPreferences,
): Promise<void> => {
  await axios.put(`/preferences`, preferences);
};
