import { DateTime } from "luxon";
import Table from "react-bootstrap/Table";

import DatedLineGraph from "~/components/DatedLineGraph";
import { ExerciseStatistics, RepPersonalBest } from "~/api/analysis";

interface Props {
  stats: ExerciseStatistics;
}

const renderRow = (row: RepPersonalBest) => {
  const recorded = DateTime.fromSQL(row.recorded);

  return (
    <tr key={row.reps}>
      <td>{row.reps}</td>
      <td>{row.weight}</td>
      <td>{recorded.toLocaleString(DateTime.DATE_MED_WITH_WEEKDAY)}</td>
    </tr>
  );
};

const Statistics = (props: Props) => {
  const { stats } = props;

  return (
    <>
      <DatedLineGraph
        title="Estimated 1RM over Time"
        label="Estimated 1RM"
        labels={stats.estimatedMaxes.map((r) => DateTime.fromISO(r.recorded))}
        values={stats.estimatedMaxes.map((r) => r.estimate)}
        yLabel="Estimated Max (kg)"
      />

      <h4 className="mt-3 align-center">All-Time Personal Bests</h4>

      <Table striped hover>
        <thead>
          <tr>
            <th>Reps</th>
            <th>Weight (kg)</th>
            <th>Date</th>
          </tr>
        </thead>
        <tbody>{stats.repPersonalBests.map(renderRow)}</tbody>
      </Table>
    </>
  );
};

export default Statistics;
