import { useState, useEffect, FormEvent } from "react";
import { AxiosError } from "axios";
import Row from "react-bootstrap/Row";
import Col from "react-bootstrap/Col";
import Button from "react-bootstrap/Button";
import Form from "react-bootstrap/Form";
import FloatingLabel from "react-bootstrap/FloatingLabel";
import { DateTime } from "luxon";
import "chartjs-adapter-luxon";

import {
  useBodyweightByDate,
  useUpdateBodyweight,
  useDeleteBodyweight,
} from "~/hooks/useBodyweight";
import ReactQueryStatefulSubmit from "~/components/ReactQueryStatefulSubmit";

const BodyweightForm = () => {
  const [recorded, setRecorded] = useState("");
  const [bodyweight, setBodyweight] = useState("");

  const defaultValue = DateTime.now().toISODate();
  const resolvedValue = recorded || defaultValue;

  const { data: bodyweightData } = useBodyweightByDate(resolvedValue);
  const updateBodyweight = useUpdateBodyweight();
  const deleteBodyweight = useDeleteBodyweight();

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    updateBodyweight.mutate({
      recorded: resolvedValue,
      bodyweight: parseFloat(bodyweight),
    });
  };

  const handleDelete = () => {
    deleteBodyweight.mutate(resolvedValue);
    setBodyweight("");
  };

  useEffect(() => {
    if (bodyweightData) {
      setBodyweight(bodyweightData.bodyweight.toString());
    } else {
      setBodyweight("");
    }
  }, [bodyweightData]);

  return (
    <Form onSubmit={handleSubmit}>
      <FloatingLabel controlId="floatingRecorded" label="Recorded Date">
        <Form.Control
          type="date"
          value={resolvedValue}
          className="mb-2"
          onChange={(e) => setRecorded(e.target.value)}
          required
        />
      </FloatingLabel>

      <FloatingLabel controlId="floatingBodyweight" label="Bodyweight">
        <Form.Control
          type="number"
          step={0.1}
          value={bodyweight}
          className="mb-2"
          onChange={(e) => setBodyweight(e.target.value)}
          required
        />
      </FloatingLabel>

      <Row>
        <Col>
          <ReactQueryStatefulSubmit
            state={updateBodyweight.status}
            error={updateBodyweight.error as AxiosError}
          />
        </Col>
        <Col>
          <Button variant="danger" onClick={handleDelete} className="w-100">
            Delete
          </Button>
        </Col>
      </Row>
    </Form>
  );
};

export default BodyweightForm;
