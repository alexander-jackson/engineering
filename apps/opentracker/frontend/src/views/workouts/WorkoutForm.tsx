import { useState, useEffect, FormEvent } from "react";
import Button from "react-bootstrap/Button";
import Form from "react-bootstrap/Form";
import InputGroup from "react-bootstrap/InputGroup";
import { PlusCircle, TrashFill } from "react-bootstrap-icons";
import { DateTime } from "luxon";

import AddExerciseModal from "~/views/workouts/AddExerciseModal";
import WorkoutView from "~/views/workouts/WorkoutView";
import {
  useWorkout,
  useUpdateWorkout,
  useDeleteWorkout,
} from "~/hooks/useWorkout";
import { Exercise } from "~/shared/types";
import ConfirmationModal from "~/components/ConfirmationModal";

const WorkoutForm = () => {
  const [recorded, setRecorded] = useState("");
  const [exercises, setExercises] = useState<Exercise[]>([]);
  const [showModal, setShowModal] = useState(false);
  const [showDeleteConfirmation, setShowDeleteConfirmation] = useState(false);
  const [editExercise, setEditExercise] = useState<
    { exercise: Exercise; index: number } | undefined
  >();

  const defaultValue = DateTime.now().toISODate();
  const resolvedValue = recorded || defaultValue;

  const { data: workoutData } = useWorkout(resolvedValue);
  const updateWorkout = useUpdateWorkout();
  const deleteWorkout = useDeleteWorkout();

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    updateWorkout.mutate({ recorded: resolvedValue, exercises });
  };

  const handleWorkoutDelete = () => {
    deleteWorkout.mutate(resolvedValue);
    setRecorded("");
    setExercises([]);
  };

  // Sync exercises from query data
  useEffect(() => {
    if (workoutData) {
      setExercises(workoutData);
    } else {
      setExercises([]);
    }
  }, [workoutData]);

  return (
    <Form onSubmit={handleSubmit}>
      <AddExerciseModal
        show={showModal}
        onHide={() => {
          setShowModal(false);
          setEditExercise(undefined);
        }}
        exercises={exercises}
        setExercises={setExercises}
        currentDate={resolvedValue}
        saveWorkout={(updatedExercises) =>
          updateWorkout.mutate({
            recorded: resolvedValue,
            exercises: updatedExercises,
          })
        }
        editExercise={editExercise}
      />
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
              onClick={() => setShowModal(true)}
              aria-label="add-exercise"
            >
              <PlusCircle />
            </Button>
          )}
        </InputGroup>
      </Form.Group>

      <WorkoutView
        exercises={exercises}
        setExercises={setExercises}
        recorded={resolvedValue}
        onEdit={(exercise, index) => {
          setEditExercise({ exercise, index });
          setShowModal(true);
        }}
        onDelete={(index) => {
          const newExercises = [...exercises];
          newExercises.splice(index, 1);

          // If this is the last exercise, delete the whole workout
          if (newExercises.length === 0) {
            handleWorkoutDelete();
          } else {
            // Otherwise, update exercises and save
            setExercises(newExercises);
            updateWorkout.mutate({
              recorded: resolvedValue,
              exercises: newExercises,
            });
          }
        }}
      />
    </Form>
  );
};

export default WorkoutForm;
