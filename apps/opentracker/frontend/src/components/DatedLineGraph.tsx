import { ChangeEvent, useState } from "react";
import Container from "react-bootstrap/Container";
import Form from "react-bootstrap/Form";
import InputGroup from "react-bootstrap/InputGroup";
import { Chart } from "react-chartjs-2";
import { DateTime, Duration } from "luxon";
import { TooltipItem, ChartData, ChartOptions } from "chart.js";

import { findLowerBoundIndex, findUpperBoundIndex } from "~/shared/utils";

interface Props {
  title: string;
  label: string;
  labels: Array<DateTime>;
  values: Array<number>;
  yLabel: string;
  defaultMinDate?: DateTime;
}

enum TimePeriodMode {
  Fixed,
  Custom,
}

const NOW = DateTime.now();
const EPOCH = DateTime.fromMillis(0);

const ONE_MONTH_DURATION = Duration.fromObject({ months: 1 });
const THREE_MONTH_DURATION = Duration.fromObject({ months: 3 });
const SIX_MONTH_DURATION = Duration.fromObject({ months: 6 });

const THREE_MONTHS_AGO = NOW.minus(THREE_MONTH_DURATION);

const secondsFromEpoch = (value: DateTime | Duration): number => {
  if (value instanceof DateTime) {
    return value.toSeconds();
  }

  return NOW.minus(value).toSeconds();
};

const getSubslice = <T,>(arr: Array<T>, lower?: number, upper?: number) => {
  if (lower === undefined && upper === undefined) {
    return [];
  }

  return arr.slice(lower, upper);
};

const computeNumberOfWeeksInRange = (data: Array<DateTime>) => {
  if (data.length < 2) {
    return undefined;
  }

  return data[data.length - 1].diff(data[0], "weeks").toObject().weeks;
};

const computeLeastSquaresRegression = (
  labels: Array<DateTime>,
  values: Array<number>,
) => {
  const n = labels.length;

  const timestamps = labels.map((label) => label.toSeconds());

  const xx = timestamps.map((timestamp) => timestamp * timestamp);
  const xy = timestamps.map((e, i) => e * values[i]);

  const sum = (x: Array<number>) => x.reduce((prev, curr) => prev + curr, 0);

  const sx = sum(timestamps);
  const sy = sum(values);
  const sxx = sum(xx);
  const sxy = sum(xy);

  const m = (n * sxy - sx * sy) / (n * sxx - sx * sx);
  const b = (sy - m * sx) / n;

  return [m, b];
};

const computePredictions = (values: Array<DateTime>, m: number, b: number) => {
  if (values.length <= 2) {
    return [];
  }

  return values.map((value) => m * value.toSeconds() + b);
};

const DatedLineGraph = (props: Props) => {
  const defaultMinDate = props.defaultMinDate || THREE_MONTHS_AGO;

  const [minDate, setMinDate] = useState(defaultMinDate);
  const [mode, setMode] = useState(TimePeriodMode.Fixed);
  const [customMinDate, setCustomMinDate] = useState(defaultMinDate);
  const [customMaxDate, setCustomMaxDate] = useState(NOW);

  const resolvedMinDate =
    mode === TimePeriodMode.Fixed ? minDate : customMinDate;
  const resolvedMaxDate = mode === TimePeriodMode.Fixed ? NOW : customMaxDate;

  const lowerBound = findLowerBoundIndex(props.labels, resolvedMinDate);
  const upperBound = findUpperBoundIndex(props.labels, resolvedMaxDate);

  const labels = getSubslice(props.labels, lowerBound, upperBound);
  const data = getSubslice(props.values, lowerBound, upperBound);

  const weeks = computeNumberOfWeeksInRange(labels);

  const [m, b] = computeLeastSquaresRegression(labels, data);
  const predictions = computePredictions(labels, m, b);

  const generateTooltipTitle = (points: Array<TooltipItem<"line">>): string => {
    return DateTime.fromMillis(points[0].parsed.x).toLocaleString(
      DateTime.DATE_MED_WITH_WEEKDAY,
    );
  };

  const generateTooltipLabel = (label: TooltipItem<"line">): string => {
    const element = label.raw as number;
    const rounded = element.toFixed(1);
    return `${props.label}: ${rounded}kg`;
  };

  const buildChartData = (): ChartData<"line"> => {
    return {
      labels,
      datasets: [
        {
          label: props.label,
          data,
          fill: "origin",
          backgroundColor: "rgba(227, 185, 127, 0.3)",
        },
        {
          label: "Trend",
          data: predictions,
          pointRadius: 0,
        },
      ],
    };
  };

  const onTimePeriodChange = (event: ChangeEvent<HTMLSelectElement>) => {
    const value = event.target.value;

    if (value === "Custom") {
      setMode(TimePeriodMode.Custom);
      return;
    }

    setMode(TimePeriodMode.Fixed);
    setMinDate(DateTime.fromSeconds(parseInt(value)));
  };

  const buildChartOptions = (): ChartOptions<"line"> => {
    const unit = weeks === undefined || weeks < 4 ? "day" : "week";

    return {
      scales: {
        x: {
          type: "time",
          time: {
            unit,
            displayFormats: { week: "dd MMM" },
          },
        },
        y: {
          title: { display: true, text: props.yLabel },
        },
      },
      plugins: {
        legend: { display: true },
        title: { display: true, text: props.title },
        tooltip: {
          callbacks: {
            title: generateTooltipTitle,
            label: generateTooltipLabel,
          },
        },
      },
      responsive: true,
      maintainAspectRatio: false,
    };
  };

  return (
    <Container>
      {data.length > 0 ? (
        <div
          id="canvas-wrapper"
          style={{ position: "relative", height: "60vh" }}
        >
          <Chart
            type="line"
            options={buildChartOptions()}
            data={buildChartData()}
            className="my-3"
          />
        </div>
      ) : (
        <h4 className="py-3 text-center">No historical data to show</h4>
      )}

      <InputGroup className="mb-3">
        <InputGroup.Text>Time Period</InputGroup.Text>
        <Form.Select
          defaultValue={secondsFromEpoch(defaultMinDate)}
          onChange={onTimePeriodChange}
          aria-label="Graph time period"
        >
          <option value={secondsFromEpoch(ONE_MONTH_DURATION)}>1 Month</option>
          <option value={secondsFromEpoch(THREE_MONTH_DURATION)}>
            3 Months
          </option>
          <option value={secondsFromEpoch(SIX_MONTH_DURATION)}>6 Months</option>
          <option value={secondsFromEpoch(EPOCH)}>All Time</option>
          <option>Custom</option>
        </Form.Select>
      </InputGroup>

      {mode === TimePeriodMode.Custom && (
        <>
          <InputGroup className="my-2">
            <InputGroup.Text>Start</InputGroup.Text>
            <Form.Control
              type="date"
              value={customMinDate.toISODate()}
              onChange={(e) =>
                setCustomMinDate(DateTime.fromISO(e.target.value))
              }
              required
            />
          </InputGroup>
          <InputGroup className="my-2">
            <InputGroup.Text>End</InputGroup.Text>
            <Form.Control
              type="date"
              value={customMaxDate.toISODate()}
              onChange={(e) =>
                setCustomMaxDate(DateTime.fromISO(e.target.value))
              }
              required
            />
          </InputGroup>
        </>
      )}
    </Container>
  );
};

export default DatedLineGraph;
