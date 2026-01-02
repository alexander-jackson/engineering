import Container from "react-bootstrap/Container";

import Title from "~/components/Title";
import WorkoutForm from "~/views/workouts/WorkoutForm";

const Workout = () => {
  return (
    <Container>
      <Title value="Workout" />

      <WorkoutForm />
    </Container>
  );
};

export default Workout;
