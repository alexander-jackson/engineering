import Container from "react-bootstrap/Container";
import "chartjs-adapter-luxon";

import Title from "~/components/Title";
import BodyweightForm from "~/views/bodyweight/BodyweightForm";
import HistoryGraph from "~/views/bodyweight/HistoryGraph";

const Bodyweight = () => {
  return (
    <Container>
      <Title value="Bodyweight" />

      <BodyweightForm />
      <HistoryGraph />
    </Container>
  );
};

export default Bodyweight;
