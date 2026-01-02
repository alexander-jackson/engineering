import Container from "react-bootstrap/Container";
import Tab from "react-bootstrap/Tab";
import Tabs from "react-bootstrap/Tabs";

import Title from "~/components/Title";
import PreferencesForm from "~/views/profile/PreferencesForm";
import UpdatePasswordForm from "~/views/profile/UpdatePasswordForm";

const Profile = () => {
  return (
    <Container>
      <Title value="Profile" />

      <Tabs defaultActiveKey="security" className="mb-3">
        <Tab eventKey="security" title="Security">
          <UpdatePasswordForm />
        </Tab>

        <Tab eventKey="preferences" title="Preferences">
          <PreferencesForm />
        </Tab>
      </Tabs>
    </Container>
  );
};

export default Profile;
