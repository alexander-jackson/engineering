import { Route, Routes, Navigate, useLocation } from "react-router-dom";

import Welcome from "~/views/Welcome";
import Register from "~/views/Register";
import Login from "~/views/Login";
import Logout from "~/views/Logout";
import EmailVerification from "~/views/EmailVerification";
import Dashboard from "~/views/Dashboard";
import Bodyweight from "~/views/bodyweight/index";
import Workouts from "~/views/workouts/index";
import Analysis from "~/views/analysis/index";
import Profile from "~/views/profile/index";
import HistoricalWorkouts from "~/views/HistoricalWorkouts";
import VerifyEmail from "~/views/VerifyEmail";

import { useAppSelector } from "~/store/hooks";

const AuthorisedRoute = ({ children }: { children: JSX.Element }) => {
  const user = useAppSelector((state) => state.user);
  const location = useLocation();

  if (!user.token) {
    return <Navigate to="/login" replace state={{ path: location.pathname }} />;
  }

  return children;
};

const Router = () => {
  return (
    <Routes>
      <Route path="/" element={<Welcome />} />
      <Route path="/login" element={<Login />} />
      <Route path="/logout" element={<Logout />} />
      <Route path="/register" element={<Register />} />
      <Route path="/email-verification" element={<EmailVerification />} />
      <Route
        path="/dashboard"
        element={
          <AuthorisedRoute>
            <Dashboard />
          </AuthorisedRoute>
        }
      />
      <Route
        path="/profile"
        element={
          <AuthorisedRoute>
            <Profile />
          </AuthorisedRoute>
        }
      />
      <Route
        path="/bodyweight"
        element={
          <AuthorisedRoute>
            <Bodyweight />
          </AuthorisedRoute>
        }
      />
      <Route
        path="/workouts"
        element={
          <AuthorisedRoute>
            <Workouts />
          </AuthorisedRoute>
        }
      />
      <Route
        path="/analysis"
        element={
          <AuthorisedRoute>
            <Analysis />
          </AuthorisedRoute>
        }
      />
      <Route
        path="/historical-workouts"
        element={
          <AuthorisedRoute>
            <HistoricalWorkouts />
          </AuthorisedRoute>
        }
      />
      <Route path="/verify-email/:id" element={<VerifyEmail />} />
    </Routes>
  );
};

export default Router;
