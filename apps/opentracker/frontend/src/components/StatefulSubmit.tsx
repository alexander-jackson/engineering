import Button from "react-bootstrap/Button";
import Spinner from "react-bootstrap/Spinner";
import { CloudCheck } from "react-bootstrap-icons";

import { RequestState } from "~/store/types";

interface Props {
  text?: string;
  variant?: string;
  switch?: RequestState;
  reset: () => void;
}

const StatefulSubmit = (props: Props) => {
  const state = props.switch;

  return (
    <>
      <Button
        variant={props.variant || "primary"}
        type="submit"
        className="w-100"
        onBlur={() => props.reset()}
      >
        {props.text || "Submit"}
        {state === RequestState.Pending && (
          <Spinner
            animation="border"
            role="status-pending"
            size="sm"
            className="mx-2"
          />
        )}
        {state === RequestState.Persisted && (
          <CloudCheck size={20} className="mx-2" role="status-persisted" />
        )}
      </Button>
    </>
  );
};

export default StatefulSubmit;
