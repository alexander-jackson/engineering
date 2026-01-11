import { useState, useEffect, ChangeEvent, FormEvent } from "react";
import { AxiosError } from "axios";
import Container from "react-bootstrap/Container";
import Form from "react-bootstrap/Form";

import { RepSetNotation } from "~/api/preferences";
import {
  useUserPreferences,
  useUpdatePreferences,
} from "~/hooks/usePreferences";
import ReactQueryStatefulSubmit from "~/components/ReactQueryStatefulSubmit";

const PreferencesForm = () => {
  const [repSetNotation, setRepSetNotation] = useState<RepSetNotation>(
    RepSetNotation.SetsThenReps,
  );

  const { data: preferences } = useUserPreferences();
  const updatePreferences = useUpdatePreferences();

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    updatePreferences.mutate({ repSetNotation });
  };

  const handleChange = (event: ChangeEvent<HTMLInputElement>) => {
    const variant =
      RepSetNotation[event.target.value as keyof typeof RepSetNotation];
    setRepSetNotation(variant);
  };

  useEffect(() => {
    if (preferences) {
      setRepSetNotation(preferences.repSetNotation);
    }
  }, [preferences]);

  return (
    <Container>
      <Form onSubmit={handleSubmit}>
        <Form.Group>
          <Form.Label>How should 3 sets of 6 reps be represented?</Form.Label>
          <Form.Check
            label="3x6"
            name="repSetNotation"
            value={RepSetNotation.SetsThenReps}
            type="radio"
            onChange={handleChange}
            checked={repSetNotation === RepSetNotation.SetsThenReps}
          />
          <Form.Check
            label="6x3"
            name="repSetNotation"
            value={RepSetNotation.RepsThenSets}
            type="radio"
            onChange={handleChange}
            checked={repSetNotation === RepSetNotation.RepsThenSets}
          />
        </Form.Group>
        <ReactQueryStatefulSubmit
          state={updatePreferences.status}
          error={updatePreferences.error as AxiosError}
        />
      </Form>
    </Container>
  );
};

export default PreferencesForm;
