import axios, { AxiosInstance, AxiosRequestConfig } from "axios";
import React from "react";
import ReactDOM from "react-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter } from "react-router-dom";
import { Provider } from "react-redux";
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  TimeScale,
  LineController,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler,
  ArcElement,
} from "chart.js";
import App from "~/App";
import store from "~/store";

ChartJS.register(
  CategoryScale,
  LinearScale,
  TimeScale,
  LineController,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler,
  ArcElement,
);

const applyAxiosSettings = (axios: AxiosInstance) => {
  axios.defaults.baseURL =
    import.meta.env.PUBLIC_AXIOS_BASE || "http://localhost:3025/api";

  axios.interceptors.request.use((request: AxiosRequestConfig) => {
    const token = store.getState().user.token;

    if (request.headers) {
      request.headers.Authorization = `Bearer ${token}`;
    }

    return request;
  });
};

applyAxiosSettings(axios);

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5, // 5 minutes
      cacheTime: 1000 * 60 * 10, // 10 minutes
      retry: 1,
      refetchOnWindowFocus: false,
    },
    mutations: {
      retry: 0,
    },
  },
});

ReactDOM.render(
  <React.StrictMode>
    <Provider store={store}>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </QueryClientProvider>
    </Provider>
  </React.StrictMode>,
  document.getElementById("root"),
);
