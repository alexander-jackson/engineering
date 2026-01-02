interface PercentageChangeProps {
  prior: number;
  current: number;
}

const computePercentageChange = (prior: number, current: number): number => {
  return Math.round((current / prior - 1) * 100);
};

const PercentageChange = ({ prior, current }: PercentageChangeProps) => {
  const change = computePercentageChange(prior, current);

  if (change === 0) {
    return <span>(no change)</span>;
  }

  const color = change < 0 ? "red" : "green";
  const sign = change < 0 ? "-" : "+";
  const absolute = Math.abs(change);

  return (
    <span style={{ color }}>
      ({sign}
      {absolute}%)
    </span>
  );
};

export default PercentageChange;
