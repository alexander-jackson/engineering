import { useEffect, ChangeEvent, FormEvent } from "react";
import { ConnectedProps } from "react-redux";
import Container from "react-bootstrap/Container";
import Form from "react-bootstrap/Form";

import connect from "~/store/connect";
import {
  RepSetNotation,
  setRepSetNotation,
  resetRequestState,
  fetchUserPreferences,
  persistUserPreferences,
} from "~/store/reducers/userPreferencesSlice";
import StatefulSubmit from "~/components/StatefulSubmit";

const connector = connect((state) => ({
  userPreferences: state.userPreferences,
}));

type Props = ConnectedProps<typeof connector>;

const PreferencesForm = (props: Props) => {
  const { dispatch, userPreferences } = props;
  const { state, repSetNotation } = userPreferences;

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    dispatch(persistUserPreferences());
  };

  const handleChange = (event: ChangeEvent<HTMLInputElement>) => {
    const variant =
      RepSetNotation[event.target.value as keyof typeof RepSetNotation];
    dispatch(setRepSetNotation(variant));
  };

  useEffect(() => {
    dispatch(fetchUserPreferences());
  }, [dispatch]);

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
        <StatefulSubmit
          switch={state}
          reset={() => dispatch(resetRequestState())}
        />
      </Form>
    </Container>
  );
};

export default connector(PreferencesForm);
