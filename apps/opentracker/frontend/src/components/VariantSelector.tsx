import { Fragment } from "react";
import Button from "react-bootstrap/Button";

import { ExerciseVariant } from "~/shared/types";
import VariantIcon from "~/components/VariantIcon";

const VARIANTS = [
  ExerciseVariant.Squat,
  ExerciseVariant.Bench,
  ExerciseVariant.Deadlift,
  ExerciseVariant.Other,
];

interface Props {
  selected?: ExerciseVariant;
  onClick: (variant: ExerciseVariant) => void;
}

const renderButton = (
  variant: ExerciseVariant,
  resolved: ExerciseVariant | undefined,
  onClick: (variant: ExerciseVariant) => void,
) => {
  const active = variant === resolved;
  const label = `${variant} button` + (active ? " (selected)" : "");

  return (
    <Fragment>
      <Button
        key={variant}
        size="lg"
        variant={active ? "success" : "dark"}
        className="m-2"
        onClick={() => onClick(variant)}
        aria-label={label}
      >
        <VariantIcon variant={variant} width={60} height={60} colour="white" />
      </Button>
      <p className="small">{variant}</p>
    </Fragment>
  );
};

const VariantSelector = (props: Props) => {
  const { selected, onClick } = props;

  return (
    <div className="row pb-3">
      {VARIANTS.map((variant) => (
        <div key={variant} className="col-6 col-md-3 text-center">
          {renderButton(variant, selected, onClick)}
        </div>
      ))}
    </div>
  );
};

export default VariantSelector;
