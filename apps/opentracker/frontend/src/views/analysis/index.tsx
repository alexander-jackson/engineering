import { useState, useEffect } from "react";
import Container from "react-bootstrap/Container";
import Form from "react-bootstrap/Form";
import FloatingLabel from "react-bootstrap/FloatingLabel";

import { useUniqueExercises, useExerciseStatistics } from "~/hooks/useAnalysis";
import Statistics from "~/views/analysis/Statistics";
import { ExerciseVariant } from "~/shared/types";
import Title from "~/components/Title";
import VariantSelector from "~/components/VariantSelector";

const decideLabel = (variant: ExerciseVariant): string => {
  if (variant === ExerciseVariant.Other) {
    return "Exercise";
  }

  return "Variation";
};

const Analysis = () => {
  const [variant, setVariant] = useState<ExerciseVariant>(
    ExerciseVariant.Unknown,
  );
  const [description, setDescription] = useState("");

  const { data: uniqueExercises = [] } = useUniqueExercises(variant);
  const { data: exerciseStatistics } = useExerciseStatistics(
    variant,
    description,
  );

  const renderOption = (option: string, key: number) => {
    return (
      <option value={option} key={key}>
        {option}
      </option>
    );
  };

  // Auto-select first exercise when unique exercises are fetched
  useEffect(() => {
    if (uniqueExercises.length > 0) {
      setDescription(uniqueExercises[0]);
    }
  }, [uniqueExercises]);

  return (
    <Container>
      <Title value="Analysis" />

      <VariantSelector selected={variant} onClick={setVariant} />

      <Form>
        <FloatingLabel
          controlId="selectedExerciseLabel"
          label={decideLabel(variant)}
        >
          <Form.Select
            value={description}
            onChange={(e) => setDescription(e.target.value)}
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

export default Analysis;
