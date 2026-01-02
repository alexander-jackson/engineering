import Button from "react-bootstrap/Button";
import Card from "react-bootstrap/Card";
import Col from "react-bootstrap/Col";
import Row from "react-bootstrap/Row";
import { PencilFill, TrashFill } from "react-bootstrap-icons";

import { useAppDispatch, useAppSelector } from "~/store/hooks";
import {
  showAddExerciseModal,
  deleteExercise,
  deleteStructuredWorkout,
  putStructuredWorkout,
} from "~/store/reducers/workoutSlice";
import { setFromExercise } from "~/store/reducers/pendingExerciseSlice";
import { Exercise } from "~/shared/types";
import ExertionIndicator from "~/components/ExertionIndicator";
import VariantIcon from "~/components/VariantIcon";

interface Props {
  exercises: Array<Exercise>;
  recorded: string;
}

const calculateOneRepMax = (exercise: Exercise): number => {
  const { weight, reps, rpe } = exercise;
  const normalisedRpe = rpe || 8;

  const numerator = 100 * weight;
  const denominator =
    48.8 + 53.8 * Math.exp(-0.075 * (reps + (10 - normalisedRpe)));

  return numerator / denominator;
};

interface EditButtonProps {
  index: number;
}

const EditButton = (props: EditButtonProps) => {
  const dispatch = useAppDispatch();
  const exercises = useAppSelector((state) => state.workout.exercises);

  const onClick = () => {
    // Set the selected index
    const payload = { exercise: exercises[props.index], index: props.index };
    dispatch(setFromExercise(payload));

    // Display the modal
    dispatch(showAddExerciseModal());
  };

  return (
    <Button
      variant="light"
      size="sm"
      className="mx-2 float-end"
      onClick={onClick}
    >
      <PencilFill />
    </Button>
  );
};

const WorkoutView = (props: Props) => {
  const { exercises, recorded } = props;
  const dispatch = useAppDispatch();

  const handleExerciseDeletion = (index: number) => {
    // Delete the exercise from the view
    dispatch(deleteExercise(index));

    // If this is the only exercise, delete the whole workout on the backend
    if (exercises.length === 1) {
      dispatch(deleteStructuredWorkout(recorded));
      return;
    }

    // Otherwise, persist the current state after deleting
    dispatch(putStructuredWorkout(recorded));
  };

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
                    onClick={() => handleExerciseDeletion(index)}
                  >
                    <TrashFill />
                  </Button>
                  <EditButton index={index} />
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
