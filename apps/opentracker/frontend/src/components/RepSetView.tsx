import { RepSetNotation } from "~/api/preferences";
import { useUserPreferences } from "~/hooks/usePreferences";

interface Props {
  reps: number;
  sets: number;
}

const RepSetView = (props: Props) => {
  const { data: preferences } = useUserPreferences();
  const repSetNotation =
    preferences?.repSetNotation || RepSetNotation.SetsThenReps;

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

export default RepSetView;
