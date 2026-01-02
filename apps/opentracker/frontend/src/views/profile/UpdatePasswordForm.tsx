import axios, { AxiosError } from "axios";
import Container from "react-bootstrap/Container";
import Form from "react-bootstrap/Form";
import FloatingLabel from "react-bootstrap/FloatingLabel";
import { useForm } from "react-hook-form";
import { useMutation } from "@tanstack/react-query";

import ReactQueryStatefulSubmit from "~/components/ReactQueryStatefulSubmit";

interface FormState {
  currentPassword: string;
  newPassword: string;
  repeatPassword: string;
}

const UpdatePasswordForm = () => {
  const { register, handleSubmit } = useForm<FormState>();
  const mutation = useMutation((payload: FormState) => {
    return axios.post(`/profile/update-password`, payload);
  });

  return (
    <Container>
      <h4>Update Password</h4>

      <Form
        onSubmit={handleSubmit((content: FormState) =>
          mutation.mutate(content),
        )}
      >
        <Form.Group>
          <FloatingLabel controlId="currentLabel" label="Current">
            <Form.Control
              {...register("currentPassword")}
              className="mb-2"
              type="password"
              required
            />
          </FloatingLabel>
        </Form.Group>
        <Form.Group>
          <FloatingLabel controlId="newLabel" label="New">
            <Form.Control
              {...register("newPassword")}
              className="mb-2"
              type="password"
              required
            />
          </FloatingLabel>
        </Form.Group>
        <Form.Group>
          <FloatingLabel controlId="repeatLabel" label="Repeat">
            <Form.Control
              {...register("repeatPassword")}
              className="mb-2"
              type="password"
              required
            />
          </FloatingLabel>
        </Form.Group>

        <ReactQueryStatefulSubmit
          state={mutation.status}
          error={mutation.error as AxiosError}
        />
      </Form>
    </Container>
  );
};

export default UpdatePasswordForm;
