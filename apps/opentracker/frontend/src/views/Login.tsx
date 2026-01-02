import { useEffect } from "react";
import axios, { AxiosError } from "axios";
import { ConnectedProps } from "react-redux";
import { Link, useNavigate, useLocation } from "react-router-dom";
import { useMutation } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import Button from "react-bootstrap/Button";
import Container from "react-bootstrap/Container";
import Form from "react-bootstrap/Form";
import FloatingLabel from "react-bootstrap/FloatingLabel";

import connect from "~/store/connect";
import { setToken } from "~/store/reducers/userSlice";
import { fetchUserPreferences } from "~/store/reducers/userPreferencesSlice";
import Title from "~/components/Title";
import ReactQueryStatefulSubmit from "~/components/ReactQueryStatefulSubmit";

interface LocationState {
  path: string;
}

const connector = connect((state) => ({
  user: state.user,
}));

type Props = ConnectedProps<typeof connector>;

interface LoginFormState {
  email: string;
  password: string;
}

const Login = (props: Props) => {
  const { user, dispatch } = props;
  const { token } = user;
  const { register, handleSubmit } = useForm<LoginFormState>();

  const navigate = useNavigate();
  const locationState = useLocation().state as LocationState;

  const login = useMutation(
    (payload: LoginFormState) => {
      return axios.put("/login", payload).then((res) => res.data);
    },
    {
      onSuccess: (token: string) => {
        dispatch(setToken(token));
      },
    },
  );

  useEffect(() => {
    if (token) {
      dispatch(fetchUserPreferences());
      navigate(locationState?.path || "/email-verification");
    }
  }, [dispatch, navigate, locationState?.path, token]);

  return (
    <Container>
      <div className="mx-auto" style={{ width: "340px" }}>
        <Title value="Login" />

        <Form
          onSubmit={handleSubmit((content: LoginFormState) =>
            login.mutate(content),
          )}
        >
          <Form.Group>
            <FloatingLabel controlId="floatingEmail" label="Email address">
              <Form.Control
                {...register("email")}
                className="mb-2"
                type="email"
                required
              />
            </FloatingLabel>
          </Form.Group>
          <Form.Group>
            <FloatingLabel controlId="floatingPassword" label="Password">
              <Form.Control
                {...register("password")}
                className="mb-2"
                type="password"
                required
              />
            </FloatingLabel>
          </Form.Group>

          <ReactQueryStatefulSubmit
            text="Login"
            variant="success"
            state={login.status}
            error={login.error as AxiosError}
          />

          <hr className="my-3" />

          <Link to="/register">
            <Button variant="outline-secondary" className="w-100">
              New? Create An Account
            </Button>
          </Link>
        </Form>
      </div>
    </Container>
  );
};

export default connector(Login);
