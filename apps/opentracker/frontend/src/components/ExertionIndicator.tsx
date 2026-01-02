import ProgressBar from "react-bootstrap/ProgressBar";

interface Props {
  rpe?: number;
}

const decideVariant = (rpe?: number): string => {
  if (rpe === undefined || rpe <= 7) {
    return "success";
  }

  if (rpe < 8.5) {
    return "warning";
  }

  return "danger";
};

const decideTextColour = (variant: string): string => {
  switch (variant) {
    case "warning":
      return "text-dark";
    default:
      return "text-light";
  }
};

const ExertionIndicator = (props: Props) => {
  const { rpe } = props;

  if (!rpe) {
    return null;
  }

  const variant = decideVariant(rpe);

  const label = `RPE: ${rpe}`;
  const textColour = decideTextColour(variant);
  const styledLabel = <span className={textColour}>{label}</span>;

  return (
    <ProgressBar
      className="w-75 mx-auto mb-2"
      now={rpe}
      label={styledLabel}
      max={10}
      variant={decideVariant(rpe)}
    />
  );
};

export default ExertionIndicator;
