import { Navigate } from "react-router-dom";

import { useAppDispatch } from "~/store/hooks";
import { logout } from "~/store/reducers/userSlice";

const Logout = () => {
  const dispatch = useAppDispatch();

  dispatch(logout());

  return <Navigate to="/" />;
};

export default Logout;
