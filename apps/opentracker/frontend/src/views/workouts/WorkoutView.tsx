import Button from "react-bootstrap/Button";
import Card from "react-bootstrap/Card";
import Col from "react-bootstrap/Col";
import Row from "react-bootstrap/Row";
import { PencilFill, TrashFill } from "react-bootstrap-icons";

import { Exercise } from "~/shared/types";
import ExertionIndicator from "~/components/ExertionIndicator";
import VariantIcon from "~/components/VariantIcon";

interface Props {
  exercises: Exercise[];
  setExercises: (exercises: Exercise[]) => void;
  recorded: string;
  onEdit: (exercise: Exercise, index: number) => void;
  onDelete: (index: number) => void;
}

const calculateOneRepMax = (exercise: Exercise): number => {
  const { weight, reps, rpe } = exercise;
  const normalisedRpe = rpe || 8;

  const numerator = 100 * weight;
  const denominator =
    48.8 + 53.8 * Math.exp(-0.075 * (reps + (10 - normalisedRpe)));

  return numerator / denominator;
};

const WorkoutView = (props: Props) => {
  const { exercises, onEdit, onDelete } = props;

  return (
    <>
      {exercises.map((exercise, index) => (
        <Card key={index} className="my-3">
          <Card.Body>
            <Card.Title>
              <Row>
                <Col>
                  <VariantIcon
                    variant={exercise.variant}
                    width={25}
                    height={25}
                    colour="black"
                  />{" "}
                  {exercise.description}
                </Col>
                <Col>
                  <Button
                    variant="outline-danger"
                    size="sm"
                    className="mx-2 float-end"
                    onClick={() => onDelete(index)}
                  >
                    <TrashFill />
                  </Button>
                  <Button
                    variant="light"
                    size="sm"
                    className="mx-2 float-end"
                    onClick={() => onEdit(exercise, index)}
                  >
                    <PencilFill />
                  </Button>
                </Col>
              </Row>
            </Card.Title>
            <Row>
              <Col>Weight: {exercise.weight}kg</Col>
              <Col>Reps: {exercise.reps}</Col>
              <Col>Sets: {exercise.sets}</Col>
            </Row>
            <hr />
            <ExertionIndicator rpe={exercise.rpe} />
            <Card.Text>
              Estimated Max: {calculateOneRepMax(exercise).toFixed(0)}
            </Card.Text>
          </Card.Body>
        </Card>
      ))}
    </>
  );
};

export default WorkoutView;
