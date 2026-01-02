import { Fragment } from "react";
import { AxiosError } from "axios";
import Alert from "react-bootstrap/Alert";
import Button from "react-bootstrap/Button";
import Spinner from "react-bootstrap/Spinner";

type QueryState = "loading" | "error" | "success" | "idle";

interface ServerError {
  message?: string;
}

interface Props {
  text?: string;
  variant?: string;
  state: QueryState;
  error?: AxiosError;
}

const ReactQueryStatefulSubmit = ({ text, variant, state, error }: Props) => {
  const errorMessage = (error?.response?.data as ServerError)?.message;

  return (
    <Fragment>
      <Button variant={variant || "primary"} type="submit" className="w-100">
        {text || "Submit"}
        {state === "loading" && (
          <Spinner
            animation="border"
            role="status-pending"
            size="sm"
            className="mx-2"
          />
        )}
      </Button>
      {state === "error" && errorMessage && (
        <Alert variant="danger" className="my-3 text-center">
          {errorMessage}
        </Alert>
      )}
    </Fragment>
  );
};

export default ReactQueryStatefulSubmit;
