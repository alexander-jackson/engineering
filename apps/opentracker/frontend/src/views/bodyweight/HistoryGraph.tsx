import { ConnectedProps } from "react-redux";

import connect from "~/store/connect";
import DatedLineGraph from "~/components/DatedLineGraph";
import { DateTime, Duration } from "luxon";

const connector = connect((state) => ({
  labels: state.bodyweight.labels,
  values: state.bodyweight.values.map(parseFloat),
}));

type Props = ConnectedProps<typeof connector>;

const NOW = DateTime.now();
const ONE_MONTH_DURATION = Duration.fromObject({ months: 1 });
const ONE_MONTH_AGO = NOW.minus(ONE_MONTH_DURATION);

const HistoryGraph = (props: Props) => {
  const { labels, values } = props;

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

export default connector(HistoryGraph);
