import { Fragment } from "react";
import Col from "react-bootstrap/Col";
import Row from "react-bootstrap/Row";
import axios from "axios";
import { useQuery } from "@tanstack/react-query";
import { DateTime } from "luxon";

import Title from "~/components/Title";
import DoughnutChart from "~/components/DoughnutChart";
import PercentageChange from "~/components/PercentageChange";
import VariantIcon from "~/components/VariantIcon";
import { ExerciseVariant } from "~/shared/types";

interface VolumeOverview {
  squatVolumePastWeek?: number;
  benchVolumePastWeek?: number;
  deadliftVolumePastWeek?: number;
  otherVolumePastWeek?: number;
}

interface VolumePillProps {
  name: ExerciseVariant;
  lastWeek?: number;
  weekBefore?: number;
}

const VolumePill = ({ name, lastWeek, weekBefore }: VolumePillProps) => {
  if (lastWeek === null) {
    return null;
  }

  return (
    <Col key={name}>
      <div className="d-flex align-items-center border border-secondary rounded p-3 mb-2">
        <div className="flex-shrink-0">
          <VariantIcon
            variant={name}
            width={60}
            height={60}
            className="me-3 p-1 bg-dark border border-secondary rounded"
            colour="white"
          />
        </div>
        <div className="flex-grow-1 ms-3">{name}</div>
        <div className="me-3">
          {lastWeek}kg{" "}
          {lastWeek && weekBefore ? (
            <PercentageChange prior={weekBefore} current={lastWeek} />
          ) : null}
        </div>
      </div>
    </Col>
  );
};

const anyPresent = (overview?: VolumeOverview): boolean => {
  if (!overview) {
    return false;
  }

  return !!(
    overview.squatVolumePastWeek ||
    overview.benchVolumePastWeek ||
    overview.deadliftVolumePastWeek
  );
};

const WeeklyVolumeStatistics = () => {
  const { data: lastWeek } = useQuery<VolumeOverview>(
    ["volume-overview-past-week"],
    () =>
      axios
        .get("/workouts/statistics", {
          params: { end: DateTime.now().toJSDate() },
        })
        .then((response) => response.data),
  );

  const { data: weekBefore } = useQuery<VolumeOverview>(
    ["volume-overview-week-before"],
    () =>
      axios
        .get("/workouts/statistics", {
          params: { end: DateTime.now().minus({ weeks: 1 }).toJSDate() },
        })
        .then((response) => response.data),
  );

  if (!anyPresent(lastWeek)) {
    return null;
  }

  const volumes = [
    {
      name: ExerciseVariant.Squat,
      lastWeek: lastWeek?.squatVolumePastWeek,
      weekBefore: weekBefore?.squatVolumePastWeek,
    },
    {
      name: ExerciseVariant.Bench,
      lastWeek: lastWeek?.benchVolumePastWeek,
      weekBefore: weekBefore?.benchVolumePastWeek,
    },
    {
      name: ExerciseVariant.Deadlift,
      lastWeek: lastWeek?.deadliftVolumePastWeek,
      weekBefore: weekBefore?.deadliftVolumePastWeek,
    },
    {
      name: ExerciseVariant.Other,
      lastWeek: lastWeek?.otherVolumePastWeek,
      weekBefore: weekBefore?.otherVolumePastWeek,
    },
  ];

  const segments = volumes.map((volume) => ({
    label: volume.name,
    value: volume.lastWeek,
  }));

  return (
    <Fragment>
      <Title value="Weekly Volume" />

      <DoughnutChart hoverLabel="Total volume" segments={segments} />

      <Row xs={1} lg={2}>
        {volumes.map(({ name, lastWeek, weekBefore }) => (
          <VolumePill name={name} lastWeek={lastWeek} weekBefore={weekBefore} />
        ))}
      </Row>
    </Fragment>
  );
};

export default WeeklyVolumeStatistics;
