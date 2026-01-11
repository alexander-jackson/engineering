import { useState, useEffect, FormEvent } from "react";
import Button from "react-bootstrap/Button";
import Col from "react-bootstrap/Col";
import Form from "react-bootstrap/Form";
import Modal from "react-bootstrap/Modal";
import Row from "react-bootstrap/Row";
import { DateTime } from "luxon";

import { useUniqueExercises } from "~/hooks/useAnalysis";
import { useLastSession } from "~/hooks/useLastSession";
import { Exercise, ExerciseVariant, LastExerciseSession } from "~/shared/types";
import Search from "~/components/Search";
import VariantSelector from "~/components/VariantSelector";

interface Props {
  show: boolean;
  onHide: () => void;
  exercises: Exercise[];
  setExercises: (exercises: Exercise[]) => void;
  currentDate: string;
  saveWorkout: (exercises: Exercise[]) => void;
  editExercise?: { exercise: Exercise; index: number };
}

interface PendingExercise {
  variant?: ExerciseVariant;
  description?: string;
  weight?: number;
  reps?: number;
  sets?: number;
  rpe?: number;
  editIndex?: number;
}

const resolveVariant = (
  specified?: ExerciseVariant,
  placeholder?: ExerciseVariant,
): ExerciseVariant | undefined => {
  if (specified === ExerciseVariant.Unknown) {
    return placeholder;
  }

  return specified || placeholder;
};

const resolve = (
  specified: PendingExercise,
  placeholder?: Exercise,
): Omit<PendingExercise, "editIndex"> & {
  variant: ExerciseVariant;
  description: string;
  weight: number;
  reps: number;
  sets: number;
} => {
  const resolvedVariant = resolveVariant(
    specified.variant,
    placeholder?.variant,
  );

  return {
    variant: resolvedVariant || ExerciseVariant.Unknown,
    description: specified.description || placeholder?.description || "",
    weight: specified.weight || placeholder?.weight || 0,
    reps: specified.reps || placeholder?.reps || 0,
    sets: specified.sets || placeholder?.sets || 0,
    rpe: specified.rpe,
  };
};

export const formatDate = (dateString: string): string => {
  const date = DateTime.fromISO(dateString);

  // Format as "Sat 3rd January"
  const dayOfWeek = date.toFormat("ccc");
  const day = date.day;
  const month = date.toFormat("MMMM");

  // Add ordinal suffix (1st, 2nd, 3rd, 4th, etc.)
  const getOrdinal = (n: number): string => {
    const s = ["th", "st", "nd", "rd"];
    const v = n % 100;
    return n + (s[(v - 20) % 10] || s[v] || s[0]);
  };

  return `${dayOfWeek} ${getOrdinal(day)} ${month}`;
};

const PreviousSessionDisplay = ({
  lastSession,
  loading,
}: {
  lastSession?: LastExerciseSession | null;
  loading?: boolean;
}) => {
  if (loading) {
    return (
      <div className="mb-3 p-3 bg-light rounded">
        <div className="text-muted">Loading previous session...</div>
      </div>
    );
  }

  if (!lastSession) {
    return null;
  }

  const { exercise } = lastSession;

  return (
    <div className="mb-3 p-3 bg-light rounded">
      <h6 className="text-dark mb-2">
        Previous Session ({formatDate(lastSession.recorded)})
      </h6>
      <Row className="text-dark">
        <Col>
          <small className="text-muted">Weight:</small>
          <div>{exercise.weight} kg</div>
        </Col>
        <Col>
          <small className="text-muted">Reps:</small>
          <div>{exercise.reps}</div>
        </Col>
        <Col>
          <small className="text-muted">Sets:</small>
          <div>{exercise.sets}</div>
        </Col>
        {exercise.rpe && (
          <Col>
            <small className="text-muted">RPE:</small>
            <div>{exercise.rpe}</div>
          </Col>
        )}
      </Row>
    </div>
  );
};

const AddExerciseModal = (props: Props) => {
  const {
    show,
    onHide,
    exercises,
    setExercises,
    currentDate,
    saveWorkout,
    editExercise,
  } = props;

  // Local form state
  const [variant, setVariant] = useState<ExerciseVariant>(
    ExerciseVariant.Unknown,
  );
  const [description, setDescription] = useState("");
  const [weight, setWeight] = useState<number | undefined>();
  const [reps, setReps] = useState<number | undefined>();
  const [sets, setSets] = useState<number | undefined>();
  const [rpe, setRpe] = useState<number | undefined>();
  const [editIndex, setEditIndex] = useState<number | undefined>();

  // Populate form when editing
  useEffect(() => {
    if (editExercise) {
      const { exercise, index } = editExercise;
      setVariant(exercise.variant);
      setDescription(exercise.description);
      setWeight(exercise.weight);
      setReps(exercise.reps);
      setSets(exercise.sets);
      setRpe(exercise.rpe);
      setEditIndex(index);
    }
  }, [editExercise]);

  const placeholder = exercises.at(-1);
  const pending: PendingExercise = {
    variant,
    description,
    weight,
    reps,
    sets,
    rpe,
    editIndex,
  };
  const resolved = resolve(pending, placeholder);

  // Fetch unique exercises and last session
  const { data: uniqueExercises = [] } = useUniqueExercises(variant);
  const { data: lastSession, isLoading: lastSessionLoading } = useLastSession(
    variant !== ExerciseVariant.Unknown ? variant : undefined,
    description && description.trim() !== "" ? description : undefined,
    editIndex === undefined ? currentDate : undefined,
  );

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    event.stopPropagation();

    const exercise: Exercise = {
      variant: resolved.variant!,
      description: resolved.description!,
      weight: resolved.weight!,
      reps: resolved.reps!,
      sets: resolved.sets!,
      rpe: resolved.rpe,
    };

    // Create new exercises array
    const newExercises = [...exercises];

    // Decide whether to edit or just add it
    if (editIndex !== undefined) {
      newExercises[editIndex] = exercise;
    } else {
      newExercises.push(exercise);
    }

    // Update exercises and save
    setExercises(newExercises);

    // Reset form and close modal
    resetForm();
    onHide();

    // Save workout with the updated exercises
    saveWorkout(newExercises);
  };

  const resetForm = () => {
    setVariant(ExerciseVariant.Unknown);
    setDescription("");
    setWeight(undefined);
    setReps(undefined);
    setSets(undefined);
    setRpe(undefined);
    setEditIndex(undefined);
  };

  const handleHide = () => {
    resetForm();
    onHide();
  };

  return (
    <Modal show={show} onHide={handleHide}>
      <Modal.Header closeButton>
        <Modal.Title>Add Exercise</Modal.Title>
      </Modal.Header>

      <Modal.Body>
        <Form onSubmit={handleSubmit}>
          <VariantSelector selected={resolved.variant} onClick={setVariant} />

          <Form.Group className="mb-3" controlId="exercise-name-input">
            <Form.Label className="text-dark">
              {resolved.variant !== ExerciseVariant.Other
                ? "Variation"
                : "Exercise"}
            </Form.Label>
            <Form.Control
              type="text"
              autoComplete="off"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={placeholder?.description}
              required={!(description || placeholder?.description)}
            />
          </Form.Group>

          <Search
            haystack={uniqueExercises}
            needle={description}
            onClick={setDescription}
          />

          <PreviousSessionDisplay
            lastSession={lastSession}
            loading={
              lastSessionLoading &&
              variant !== ExerciseVariant.Unknown &&
              !!description &&
              description.trim() !== ""
            }
          />

          <Row>
            <Col>
              <Form.Group className="mb-3" controlId="weight-input">
                <Form.Label className="text-dark">Weight (kg)</Form.Label>
                <Form.Control
                  type="number"
                  step={0.5}
                  value={weight ?? ""}
                  onChange={(e) => setWeight(parseFloat(e.target.value))}
                  placeholder={placeholder?.weight?.toString()}
                  required={!(weight || placeholder?.weight)}
                />
              </Form.Group>
            </Col>

            <Col>
              <Form.Group className="mb-3" controlId="reps-input">
                <Form.Label className="text-dark">Reps</Form.Label>
                <Form.Control
                  type="number"
                  step={1}
                  value={reps ?? ""}
                  onChange={(e) => setReps(parseFloat(e.target.value))}
                  placeholder={placeholder?.reps?.toString()}
                  required={!(reps || placeholder?.reps)}
                />
              </Form.Group>
            </Col>
          </Row>

          <Row>
            <Col>
              <Form.Group className="mb-3" controlId="sets-input">
                <Form.Label className="text-dark">Sets</Form.Label>
                <Form.Control
                  type="number"
                  step={1}
                  value={sets ?? ""}
                  onChange={(e) => setSets(parseFloat(e.target.value))}
                  placeholder={placeholder?.sets?.toString()}
                  required={!(sets || placeholder?.sets)}
                />
              </Form.Group>
            </Col>

            <Col>
              <Form.Group className="mb-3" controlId="rpe-input">
                <Form.Label className="text-dark">RPE</Form.Label>
                <Form.Control
                  type="number"
                  step={0.25}
                  value={rpe ?? ""}
                  onChange={(e) => setRpe(parseFloat(e.target.value))}
                />
              </Form.Group>
            </Col>
          </Row>

          <Button
            type="submit"
            variant="primary"
            className="w-100"
            aria-label="modal-submit"
          >
            Submit
          </Button>
        </Form>
      </Modal.Body>
    </Modal>
  );
};

export default AddExerciseModal;
