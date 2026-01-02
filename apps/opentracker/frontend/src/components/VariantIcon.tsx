import { ExerciseVariant } from "~/shared/types";

type VariantColour = "white" | "black";

interface VariantIconProps {
  variant: ExerciseVariant;
  width: number;
  height: number;
  className?: string;
  colour: VariantColour;
}

const VariantIcon = ({
  variant,
  width,
  height,
  className,
  colour,
}: VariantIconProps) => {
  const lower = variant.toLowerCase();

  const src = `/icons/${colour}_${lower}.svg`;
  const alt = `${variant} icon (${colour})`;

  return (
    <img
      src={src}
      width={width}
      height={height}
      alt={alt}
      className={className}
    />
  );
};

export default VariantIcon;
