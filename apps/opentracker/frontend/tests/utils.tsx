import { configureStore } from "@reduxjs/toolkit";
import { Provider } from "react-redux";
import { render as rtlRender } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { createMemoryHistory } from "history";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { reducer } from "~/store";

import Router from "~/Router";
import loginReducer from "~/store/loginSlice";
import userReducer from "~/store/userSlice";
import userPreferencesReducer from "~/store/userPreferencesSlice";
import workoutReducer from "~/store/workoutSlice";
import pendingExerciseReducer from "~/store/pendingExerciseSlice";

const render = (
  component,
  {
    preloadedState,
    store = configureStore({
      reducer,
      preloadedState,
    }),
    routerSettings,
    ...renderOptions
  } = {},
) => {
  const history = createMemoryHistory();
  const queryClient = new QueryClient();

  if (!routerSettings) {
    const Wrapper = ({ children }) => (
      <Provider store={store}>
        <QueryClientProvider client={queryClient}>
          {children}
        </QueryClientProvider>
      </Provider>
    );

    return rtlRender(component, { wrapper: Wrapper, ...renderOptions });
  }

  const Wrapper = ({ children }) => (
    <Provider store={store}>
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={routerSettings.initialEntries}>
          <Router>{children}</Router>
        </MemoryRouter>
      </QueryClientProvider>
    </Provider>
  );

  return rtlRender(component, { wrapper: Wrapper, ...renderOptions });
};

export const endpoint = (path: string) => {
  return `http://localhost${path}`;
};

export default render;
