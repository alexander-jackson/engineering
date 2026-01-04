import { useEffect, FormEvent } from "react";
import { ConnectedProps } from "react-redux";
import Button from "react-bootstrap/Button";
import Col from "react-bootstrap/Col";
import Form from "react-bootstrap/Form";
import Modal from "react-bootstrap/Modal";
import Row from "react-bootstrap/Row";
import { DateTime } from "luxon";

import connect from "~/store/connect";
import {
  setVariant,
  setDescription,
  setWeight,
  setReps,
  setSets,
  setRpe,
  reset,
  clearLastSession,
  PendingExercise,
  fetchLastSession,
} from "~/store/reducers/pendingExerciseSlice";
import {
  hideAddExerciseModal,
  addExercise,
  editExercise,
} from "~/store/reducers/workoutSlice";
import { fetchUniqueExercises } from "~/store/reducers/analysisSlice";
import { Exercise, ExerciseVariant, LastExerciseSession } from "~/shared/types";
import Search from "~/components/Search";
import VariantSelector from "~/components/VariantSelector";

const connector = connect((state) => ({
  workout: state.workout,
  pending: state.pendingExercise,
  uniqueExercises: state.analysis.uniqueExercises,
}));

type Props = ConnectedProps<typeof connector> & {
  currentDate: string;
};

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
): PendingExercise => {
  const resolvedVariant = resolveVariant(
    specified.variant,
    placeholder?.variant,
  );

  return {
    variant: resolvedVariant,
    description: specified.description || placeholder?.description,
    weight: specified.weight || placeholder?.weight,
    reps: specified.reps || placeholder?.reps,
    sets: specified.sets || placeholder?.sets,
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
  const { workout, pending, uniqueExercises, currentDate, dispatch } = props;

  const placeholder = workout.exercises.at(-1);
  const resolved = resolve(pending, placeholder);

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    const { variant, description, weight, reps, sets, rpe } = resolved;

    const exercise: Exercise = {
      variant: variant!,
      description: description!,
      weight: weight!,
      reps: reps!,
      sets: sets!,
      rpe,
    };

    // Decide whether to edit or just add it
    if (pending.index !== undefined) {
      const payload = { index: pending.index, exercise };
      dispatch(editExercise(payload));
    } else {
      dispatch(addExercise(exercise));
    }

    dispatch(hideAddExerciseModal());
    dispatch(reset());
  };

  const onHide = () => {
    dispatch(hideAddExerciseModal());
    dispatch(reset());
  };

  useEffect(() => {
    const variant = resolved.variant;

    if (variant === undefined || variant === ExerciseVariant.Unknown) {
      return;
    }

    dispatch(fetchUniqueExercises(variant));
  }, [resolved.variant, dispatch]);

  useEffect(() => {
    const variant = resolved.variant;
    const description = resolved.description;

    // Clear last session if conditions aren't met
    if (
      !variant ||
      variant === ExerciseVariant.Unknown ||
      !description ||
      description.trim() === "" ||
      pending.index !== undefined
    ) {
      dispatch(clearLastSession());
      return;
    }

    // Only fetch if the description matches an existing exercise from history
    if (!uniqueExercises.includes(description)) {
      dispatch(clearLastSession());
      return;
    }

    dispatch(
      fetchLastSession({
        variant,
        description,
        currentDate,
      }),
    );
  }, [
    resolved.variant,
    resolved.description,
    currentDate,
    pending.index,
    uniqueExercises,
    dispatch,
  ]);

  return (
    <Modal show={workout.displayModal} onHide={onHide}>
      <Modal.Header closeButton>
        <Modal.Title>Add Exercise</Modal.Title>
      </Modal.Header>

      <Modal.Body>
        <Form onSubmit={handleSubmit}>
          <VariantSelector
            selected={resolved.variant}
            onClick={(variant) => dispatch(setVariant(variant))}
          />

          <Form.Group className="mb-3" controlId="exercise-name-input">
            <Form.Label className="text-dark">
              {resolved.variant !== ExerciseVariant.Other
                ? "Variation"
                : "Exercise"}
            </Form.Label>
            <Form.Control
              type="text"
              autoComplete="off"
              value={pending.description}
              onChange={(e) => dispatch(setDescription(e.target.value))}
              placeholder={placeholder?.description}
              required={!(pending.description || placeholder?.description)}
            />
          </Form.Group>

          <Search
            haystack={uniqueExercises}
            needle={pending.description}
            onClick={(v) => dispatch(setDescription(v))}
          />

          <PreviousSessionDisplay
            lastSession={pending.lastSession}
            loading={pending.lastSessionLoading}
          />

          <Row>
            <Col>
              <Form.Group className="mb-3" controlId="weight-input">
                <Form.Label className="text-dark">Weight (kg)</Form.Label>
                <Form.Control
                  type="number"
                  step={0.5}
                  value={pending.weight}
                  onChange={(e) =>
                    dispatch(setWeight(parseFloat(e.target.value)))
                  }
                  placeholder={placeholder?.weight?.toString()}
                  required={!(pending.weight || placeholder?.weight)}
                />
              </Form.Group>
            </Col>

            <Col>
              <Form.Group className="mb-3" controlId="reps-input">
                <Form.Label className="text-dark">Reps</Form.Label>
                <Form.Control
                  type="number"
                  step={1}
                  value={pending.reps}
                  onChange={(e) =>
                    dispatch(setReps(parseFloat(e.target.value)))
                  }
                  placeholder={placeholder?.reps?.toString()}
                  required={!(pending.reps || placeholder?.reps)}
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
                  value={pending.sets}
                  onChange={(e) =>
                    dispatch(setSets(parseFloat(e.target.value)))
                  }
                  placeholder={placeholder?.sets?.toString()}
                  required={!(pending.sets || placeholder?.sets)}
                />
              </Form.Group>
            </Col>

            <Col>
              <Form.Group className="mb-3" controlId="rpe-input">
                <Form.Label className="text-dark">RPE</Form.Label>
                <Form.Control
                  type="number"
                  step={0.25}
                  value={pending.rpe}
                  onChange={(e) => dispatch(setRpe(parseFloat(e.target.value)))}
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

export default connector(AddExerciseModal);
