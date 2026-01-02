import { ReactNode } from "react";
import { Link } from "react-router-dom";
import Col from "react-bootstrap/Col";

interface Props {
  icon: ReactNode;
  title: string;
  body: string;
  route: string;
}

const DashboardItem = (props: Props) => {
  const { icon, title, body, route } = props;

  return (
    <>
      <Link
        to={route}
        className="d-flex mb-4"
        style={{ textDecoration: "none", color: "inherit" }}
      >
        <Col className="d-flex align-items-start p-2 border rounded">
          <div className="d-inline-flex mx-2 p-3">{icon}</div>
          <div className="p-2">
            <h4>{title}</h4>

            <p>{body}</p>
          </div>
        </Col>
      </Link>
    </>
  );
};

export default DashboardItem;
