import { useState, useEffect, FormEvent } from "react";
import { ConnectedProps } from "react-redux";
import Button from "react-bootstrap/Button";
import Form from "react-bootstrap/Form";
import InputGroup from "react-bootstrap/InputGroup";
import { PlusCircle, TrashFill } from "react-bootstrap-icons";
import { DateTime } from "luxon";

import connect from "~/store/connect";
import AddExerciseModal from "~/views/workouts/AddExerciseModal";
import WorkoutView from "~/views/workouts/WorkoutView";
import {
  showAddExerciseModal,
  putStructuredWorkout,
  deleteStructuredWorkout,
  fetchStructuredWorkout,
} from "~/store/reducers/workoutSlice";
import ConfirmationModal from "~/components/ConfirmationModal";

const connector = connect((state) => ({ exercises: state.workout.exercises }));

type Props = ConnectedProps<typeof connector>;

const WorkoutForm = (props: Props) => {
  const [recorded, setRecorded] = useState("");
  const [showDeleteConfirmation, setShowDeleteConfirmation] = useState(false);
  const { exercises, dispatch } = props;

  const defaultValue = DateTime.now().toISODate();
  const resolvedValue = recorded || defaultValue;

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    dispatch(putStructuredWorkout(resolvedValue));
  };

  const handleWorkoutDelete = () => {
    dispatch(deleteStructuredWorkout(resolvedValue));

    setRecorded("");
  };

  useEffect(() => {
    dispatch(fetchStructuredWorkout(resolvedValue));
  }, [dispatch, resolvedValue]);

  return (
    <Form onSubmit={handleSubmit}>
      <AddExerciseModal />
      <ConfirmationModal
        show={showDeleteConfirmation}
        heading="Delete Workout"
        body="This action cannot be undone."
        handleConfirmation={handleWorkoutDelete}
        closeModal={() => setShowDeleteConfirmation(false)}
      />

      <Form.Group className="mb-3" controlId="recorded-date-input">
        <InputGroup>
          <InputGroup.Text id="recorded-date-label">Date</InputGroup.Text>
          <Form.Control
            type="date"
            value={resolvedValue}
            onChange={(e) => setRecorded(e.target.value)}
            aria-label="Date"
            aria-describedby="recorded-date-label"
            required
          />
          {resolvedValue !== "" && (
            <Button
              variant="danger"
              onClick={() => setShowDeleteConfirmation(true)}
              aria-label="delete-workout"
            >
              <TrashFill />
            </Button>
          )}
          {resolvedValue !== "" && (
            <Button
              variant="primary"
              onClick={() => dispatch(showAddExerciseModal())}
              aria-label="add-exercise"
            >
              <PlusCircle />
            </Button>
          )}
        </InputGroup>
      </Form.Group>

      <WorkoutView exercises={exercises} recorded={resolvedValue} />
    </Form>
  );
};

export default connector(WorkoutForm);
