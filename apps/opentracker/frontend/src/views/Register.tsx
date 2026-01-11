import { AxiosError } from "axios";
import { Link, useNavigate } from "react-router-dom";
import { useMutation } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import Button from "react-bootstrap/Button";
import Container from "react-bootstrap/Container";
import Form from "react-bootstrap/Form";
import FloatingLabel from "react-bootstrap/FloatingLabel";

import { useAppDispatch } from "~/store/hooks";
import Title from "~/components/Title";
import ReactQueryStatefulSubmit from "~/components/ReactQueryStatefulSubmit";
import { setToken } from "~/store/reducers/userSlice";
import { register as registerApi } from "~/api/user";

interface FormState {
  email: string;
  password: string;
}

const Register = () => {
  const dispatch = useAppDispatch();
  const { register, handleSubmit } = useForm<FormState>();

  const navigate = useNavigate();

  const mutation = useMutation(registerApi, {
    onSuccess: (token: string) => {
      dispatch(setToken(token));
      navigate("/email-verification");
    },
  });

  return (
    <Container>
      <div className="mx-auto" style={{ width: "340px" }}>
        <Title value="Register" />

        <Form
          onSubmit={handleSubmit((content: FormState) => {
            mutation.mutate(content);
          })}
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
            text="Register"
            variant="success"
            state={mutation.status}
            error={mutation.error as AxiosError}
          />

          <hr className="my-3" />

          <Link to="/login">
            <Button variant="outline-secondary" className="w-100">
              Have an Account? Login
            </Button>
          </Link>
        </Form>
      </div>
    </Container>
  );
};

export default Register;
