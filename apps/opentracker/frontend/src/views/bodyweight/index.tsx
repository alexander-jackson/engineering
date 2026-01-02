import { useEffect } from "react";
import Container from "react-bootstrap/Container";
import "chartjs-adapter-luxon";

import Title from "~/components/Title";
import { useAppDispatch } from "~/store/hooks";
import { fetchAllBodyweightEntries } from "~/store/reducers/bodyweightSlice";
import BodyweightForm from "~/views/bodyweight/BodyweightForm";
import HistoryGraph from "~/views/bodyweight/HistoryGraph";

const Bodyweight = () => {
  const dispatch = useAppDispatch();

  useEffect(() => {
    dispatch(fetchAllBodyweightEntries());
  }, [dispatch]);

  return (
    <Container>
      <Title value="Bodyweight" />

      <BodyweightForm />
      <HistoryGraph />
    </Container>
  );
};

export default Bodyweight;
