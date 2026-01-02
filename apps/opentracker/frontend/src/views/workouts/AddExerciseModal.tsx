import { useEffect, FormEvent } from "react";
import { ConnectedProps } from "react-redux";
import Button from "react-bootstrap/Button";
import Col from "react-bootstrap/Col";
import Form from "react-bootstrap/Form";
import Modal from "react-bootstrap/Modal";
import Row from "react-bootstrap/Row";

import connect from "~/store/connect";
import {
  setVariant,
  setDescription,
  setWeight,
  setReps,
  setSets,
  setRpe,
  reset,
  PendingExercise,
} from "~/store/reducers/pendingExerciseSlice";
import {
  hideAddExerciseModal,
  addExercise,
  editExercise,
} from "~/store/reducers/workoutSlice";
import { fetchUniqueExercises } from "~/store/reducers/analysisSlice";
import { Exercise, ExerciseVariant } from "~/shared/types";
import Search from "~/components/Search";
import VariantSelector from "~/components/VariantSelector";

const connector = connect((state) => ({
  workout: state.workout,
  pending: state.pendingExercise,
  uniqueExercises: state.analysis.uniqueExercises,
}));

type Props = ConnectedProps<typeof connector>;

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

const AddExerciseModal = (props: Props) => {
  const { workout, pending, uniqueExercises, dispatch } = props;

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
