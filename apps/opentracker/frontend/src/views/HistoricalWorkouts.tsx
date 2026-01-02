import { useState } from "react";
import axios from "axios";
import Card from "react-bootstrap/Card";
import ListGroup from "react-bootstrap/ListGroup";
import { useQuery } from "@tanstack/react-query";
import { DateTime } from "luxon";

import Title from "~/components/Title";
import RepSetView from "~/components/RepSetView";
import ExertionIndicator from "~/components/ExertionIndicator";
import { groupByExercise } from "~/shared/utils";
import { GroupedExercise, ExerciseDetails, DatedWorkout } from "~/shared/types";
import VariantIcon from "~/components/VariantIcon";

interface DateRange {
  start: DateTime;
  end: DateTime;
}

const defaultDateRange = {
  start: DateTime.utc().minus({ weeks: 1 }),
  end: DateTime.utc(),
};

const renderGroup = (group: ExerciseDetails, index: number) => {
  const { weight, reps, sets, rpe } = group;

  return (
    <>
      <Card.Text key={index}>
        {weight}kg for <RepSetView reps={reps} sets={sets} />
      </Card.Text>
      <ExertionIndicator rpe={rpe} />
    </>
  );
};

const renderExercise = (exercise: GroupedExercise, index: number) => {
  const { variant, description, groups } = exercise;

  return (
    <ListGroup.Item key={index} className="mb-2">
      <Card.Text>
        <VariantIcon variant={variant} width={24} height={24} colour="black" />{" "}
        {description}
      </Card.Text>
      {groups.map(renderGroup)}
    </ListGroup.Item>
  );
};

const renderWorkout = (workout: DatedWorkout, index: number) => {
  const { recorded, exercises } = workout;

  const formattedDate = DateTime.fromISO(recorded).toLocaleString(
    DateTime.DATE_HUGE,
  );
  const grouped = groupByExercise(exercises);

  return (
    <Card key={index} className="m-2">
      <Card.Body className="pb-0">
        <Card.Title className="text-center">{formattedDate}</Card.Title>

        <ListGroup variant="flush">{grouped.map(renderExercise)}</ListGroup>
      </Card.Body>
    </Card>
  );
};

const HistoricalWorkouts = () => {
  const [dateRange, setDateRange] = useState<DateRange>(defaultDateRange);

  const { isLoading, data: workouts } = useQuery(
    ["workouts", dateRange],
    () => {
      const { start, end } = dateRange;

      return axios
        .get("/workouts", {
          params: { start: start.toISO(), end: end.toISO() },
        })
        .then((response) => response.data);
    },
  );

  if (isLoading) {
    return <p>Loading</p>;
  }

  return (
    <>
      <Title value="Historical Workouts" />

      {workouts.map(renderWorkout)}
    </>
  );
};

export default HistoricalWorkouts;
