import axios from "axios";

export const register = async (params: {
  email: string;
  password: string;
}): Promise<string> => {
  const response = await axios.post<string>(`/register`, {
    email: params.email,
    password: params.password,
  });
  return response.data;
};
