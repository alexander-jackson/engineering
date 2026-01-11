import DatedLineGraph from "~/components/DatedLineGraph";
import { DateTime, Duration } from "luxon";
import { useBodyweights } from "~/hooks/useBodyweight";

const NOW = DateTime.now();
const ONE_MONTH_DURATION = Duration.fromObject({ months: 1 });
const ONE_MONTH_AGO = NOW.minus(ONE_MONTH_DURATION);

const HistoryGraph = () => {
  const { data } = useBodyweights();

  const labels = data?.labels || [];
  const values = data?.values || [];

  return (
    <DatedLineGraph
      title="Bodyweight over Time"
      labels={labels.map((label) => DateTime.fromISO(label))}
      label="Bodyweight"
      values={values}
      yLabel="Bodyweight (kg)"
      defaultMinDate={ONE_MONTH_AGO}
    />
  );
};

export default HistoryGraph;
