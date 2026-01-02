import { Link } from "react-router-dom";
import Button from "react-bootstrap/Button";

const Welcome = () => {
  return (
    <>
      <div
        className="d-flex align-items-center justify-content-center"
        style={{ height: "90vh", backgroundColor: "#e0e0e0" }}
      >
        <h1
          className="px-4 display-5 text-center"
          style={{ maxWidth: "700px" }}
        >
          Track your <span style={{ color: "#429ff8" }}>bodyweight</span>,{" "}
          <span style={{ color: "#429ff8" }}>workouts</span> and{" "}
          <span style={{ color: "#429ff8" }}>competitions</span> with analysis
          of your training through a simple and modernised interface
        </h1>
      </div>

      <div
        className="d-flex flex-column align-items-center justify-content-center"
        style={{ height: "100vh", backgroundColor: "black", color: "white" }}
      >
        <h1 className="px-4 display-6 pb-3" style={{ maxWidth: "700px" }}>
          What is OpenTracker?
        </h1>

        <p className="px-4 lead text-center" style={{ maxWidth: "700px" }}>
          OpenTracker is an{" "}
          <span style={{ color: "#429ff8" }}>open source</span> application
          enabling you to record data about your training and performance. It
          provides{" "}
          <span style={{ color: "#429ff8" }}>graphs and statistics</span> to
          help you understand that data, such as how your bodyweight is trending
          or your lifts are progressing.
        </p>
      </div>

      <div
        className="d-flex flex-column align-items-center justify-content-center"
        style={{ height: "100vh", backgroundColor: "#e0e0e0" }}
      >
        <h1 className="px-4 pb-5 display-6" style={{ maxWidth: "700px" }}>
          <span style={{ color: "#429ff8" }}>Want to get started?</span>
        </h1>

        <Link to="/register">
          <Button variant="outline-dark">Register for an Account</Button>
        </Link>
      </div>
    </>
  );
};

export default Welcome;
