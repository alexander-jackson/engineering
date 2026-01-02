import { useEffect } from "react";
import { ConnectedProps } from "react-redux";
import Container from "react-bootstrap/Container";
import Form from "react-bootstrap/Form";
import FloatingLabel from "react-bootstrap/FloatingLabel";

import connect from "~/store/connect";
import {
  setVariant,
  setDescription,
  fetchUniqueExercises,
  fetchExerciseStatistics,
} from "~/store/reducers/analysisSlice";
import Statistics from "~/views/analysis/Statistics";
import { ExerciseVariant } from "~/shared/types";
import Title from "~/components/Title";
import VariantSelector from "~/components/VariantSelector";

const connector = connect((state) => ({
  analysis: state.analysis,
}));

type Props = ConnectedProps<typeof connector>;

const decideLabel = (variant: ExerciseVariant): string => {
  if (variant === ExerciseVariant.Other) {
    return "Exercise";
  }

  return "Variation";
};

const Analysis = (props: Props) => {
  const { dispatch, analysis } = props;
  const { variant, description, uniqueExercises, exerciseStatistics } =
    analysis;

  const renderOption = (option: string, key: number) => {
    return (
      <option value={option} key={key}>
        {option}
      </option>
    );
  };

  useEffect(() => {
    if (variant !== ExerciseVariant.Unknown) {
      dispatch(fetchUniqueExercises(variant));
    }
  }, [dispatch, variant]);

  useEffect(() => {
    if (variant !== ExerciseVariant.Unknown) {
      dispatch(fetchExerciseStatistics({ variant, description }));
    }
  }, [dispatch, variant, description]);

  return (
    <Container>
      <Title value="Analysis" />

      <VariantSelector
        selected={variant}
        onClick={(variant) => dispatch(setVariant(variant))}
      />

      <Form>
        <FloatingLabel
          controlId="selectedExerciseLabel"
          label={decideLabel(variant)}
        >
          <Form.Select
            value={description}
            onChange={(e) => dispatch(setDescription(e.target.value))}
          >
            {uniqueExercises.map(renderOption)}
          </Form.Select>
        </FloatingLabel>
      </Form>

      {exerciseStatistics ? (
        <Statistics stats={exerciseStatistics} />
      ) : (
        <h4 className="py-3 text-center">No historical data to show</h4>
      )}
    </Container>
  );
};

export default connector(Analysis);
