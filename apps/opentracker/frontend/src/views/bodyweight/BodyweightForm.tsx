import { useState, useEffect, FormEvent } from "react";
import axios from "axios";
import Row from "react-bootstrap/Row";
import Col from "react-bootstrap/Col";
import Button from "react-bootstrap/Button";
import Form from "react-bootstrap/Form";
import FloatingLabel from "react-bootstrap/FloatingLabel";
import { DateTime } from "luxon";
import "chartjs-adapter-luxon";

import { useAppDispatch, useAppSelector } from "~/store/hooks";
import StatefulSubmit from "~/components/StatefulSubmit";
import {
  putBodyweightEntry,
  deleteBodyweightEntry,
  resetRequestState,
} from "~/store/reducers/bodyweightSlice";

interface SpecificBodyweightRecord {
  bodyweight: number;
}

const BodyweightForm = () => {
  const [recorded, setRecorded] = useState("");
  const [bodyweight, setBodyweight] = useState("");
  const bodyweightSelector = useAppSelector((state) => state.bodyweight);
  const dispatch = useAppDispatch();

  const defaultValue = DateTime.now().toISODate();
  const resolvedValue = recorded || defaultValue;

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    dispatch(
      putBodyweightEntry({
        recorded: resolvedValue,
        bodyweight: parseFloat(bodyweight),
      }),
    );
  };

  const handleDelete = () => {
    dispatch(deleteBodyweightEntry(resolvedValue));
    setBodyweight("");
  };

  useEffect(() => {
    axios
      .get<SpecificBodyweightRecord>(`/bodyweights/${resolvedValue}`)
      .then((record) => setBodyweight(record.data.bodyweight.toString()))
      .catch((error) => {
        if (error.response?.status !== 404) {
          console.error(error);
        } else {
          setBodyweight("");
        }
      });
  }, [dispatch, resolvedValue]);

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
          <StatefulSubmit
            switch={bodyweightSelector.state}
            reset={() => dispatch(resetRequestState())}
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
