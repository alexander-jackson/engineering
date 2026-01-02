import Container from "react-bootstrap/Container";
import Row from "react-bootstrap/Row";
import {
  Calendar3,
  JournalText,
  GraphUp,
  PersonFill,
} from "react-bootstrap-icons";

import Title from "~/components/Title";
import DashboardItem from "~/components/DashboardItem";
import WeeklyVolumeStatistics from "~/components/WeeklyVolumeStatistics";

const Dashboard = () => {
  return (
    <Container>
      <Title value="Dashboard" />

      <Row xs={1} md={2} lg={4}>
        <DashboardItem
          icon={<Calendar3 size={32} />}
          title="Bodyweight"
          body="Track your changes over time to help you make weight"
          route="/bodyweight"
        />
        <DashboardItem
          icon={<JournalText size={32} />}
          title="Workouts"
          body="Watch your estimated total progress and see what works for you"
          route="/workouts"
        />
        <DashboardItem
          icon={<GraphUp size={32} />}
          title="Analysis"
          body="Details and analysis of your workouts"
          route="/analysis"
        />
        <DashboardItem
          icon={<PersonFill size={32} />}
          title="Profile"
          body="Update and view your current settings"
          route="/profile"
        />
      </Row>

      <WeeklyVolumeStatistics />
    </Container>
  );
};

export default Dashboard;
