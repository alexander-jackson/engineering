import Container from "react-bootstrap/Container";
import { Doughnut } from "react-chartjs-2";

const COLOURS = [
  "rgba(255, 99, 132, 0.2)",
  "rgba(54, 162, 235, 0.2)",
  "rgba(255, 206, 86, 0.2)",
  "rgba(75, 192, 192, 0.2)",
];

const BORDERS = [
  "rgba(255, 99, 132, 1)",
  "rgba(54, 162, 235, 1)",
  "rgba(255, 206, 86, 1)",
  "rgba(75, 192, 192, 1)",
];

interface Segment {
  label: string;
  value?: number;
}

interface DoughnutChartProps {
  hoverLabel: string;
  segments: Array<Segment>;
}

const DoughnutChart = ({ hoverLabel, segments }: DoughnutChartProps) => {
  const availableSegments = segments.filter(
    (segment) => segment.value !== null,
  );

  const labels = availableSegments.map((segment) => segment.label);
  const values = availableSegments.map((segment) => segment.value);

  const sum = values.reduce((prev, curr) => prev! + curr!, 0);
  const percentages = values.map((value) => ((value! / sum!) * 100).toFixed(1));

  const data = {
    labels,
    datasets: [
      {
        label: hoverLabel,
        data: percentages,
        backgroundColor: COLOURS,
        borderColor: BORDERS,
        borderWidth: 1,
      },
    ],
  };

  return (
    <Container className="pb-3">
      <div id="canvas-wrapper" style={{ height: "30vh" }}>
        <Doughnut
          data={data}
          options={{ responsive: true, maintainAspectRatio: false }}
        />
      </div>
    </Container>
  );
};

export default DoughnutChart;
