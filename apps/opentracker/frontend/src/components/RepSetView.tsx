import { ConnectedProps } from "react-redux";

import connect from "~/store/connect";
import { RepSetNotation } from "~/store/reducers/userPreferencesSlice";

interface ComponentState {
  repSetNotation: RepSetNotation;
}

interface ComponentProps {
  reps: number;
  sets: number;
}

const connector = connect<ComponentState, ComponentProps>((state) => ({
  repSetNotation: state.userPreferences.repSetNotation,
}));

type Props = ConnectedProps<typeof connector>;

const RepSetView = (props: Props) => {
  const { repSetNotation } = props;

  const lhs =
    repSetNotation === RepSetNotation.RepsThenSets ? props.reps : props.sets;
  const rhs =
    repSetNotation === RepSetNotation.SetsThenReps ? props.reps : props.sets;

  return (
    <>
      {lhs}x{rhs}
    </>
  );
};

export default connector(RepSetView);
