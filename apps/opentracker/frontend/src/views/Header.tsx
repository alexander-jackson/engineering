import { ConnectedProps } from "react-redux";
import { Link } from "react-router-dom";
import Container from "react-bootstrap/Container";
import Nav from "react-bootstrap/Nav";
import Navbar from "react-bootstrap/Navbar";

import connect from "~/store/connect";

const connector = connect((state) => ({ token: state.user.token }));

type Props = ConnectedProps<typeof connector>;

const getLoginOrLogout = (authorised: boolean) => {
  if (authorised) {
    return (
      <Link to="/logout" style={{ textDecoration: "none" }}>
        <Navbar.Text className="text-danger">Logout</Navbar.Text>
      </Link>
    );
  }

  return (
    <Link to="/login" style={{ textDecoration: "none" }}>
      <Navbar.Text>Login</Navbar.Text>
    </Link>
  );
};

const Header = (props: Props) => {
  const { token } = props;
  const brandRoute = token ? "/dashboard" : "/";

  return (
    <Navbar variant="dark" bg="dark" expand="lg" style={{ height: "10vh" }}>
      <Container>
        <Link to={brandRoute} style={{ textDecoration: "none" }}>
          <Navbar.Brand>
            <img
              src="/logo.svg"
              width="30"
              height="30"
              className="d-inline-block align-top"
              alt="OpenTracker logo"
            />{" "}
            OpenTracker
          </Navbar.Brand>
        </Link>
        <Nav className="ml-auto">{getLoginOrLogout(token !== undefined)}</Nav>
      </Container>
    </Navbar>
  );
};

export default connector(Header);
