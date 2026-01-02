import axios from "axios";
import { Navigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import Container from "react-bootstrap/Container";
import { EnvelopeOpenFill } from "react-bootstrap-icons";
import { DateTime } from "luxon";

import Title from "~/components/Title";

const ENABLED = false;

interface VerificationStatus {
  emailAddressUid: string;
  emailAddress: string;
  verifiedAt?: DateTime;
}

const EmailVerification = () => {
  const { data } = useQuery<VerificationStatus>(
    ["email-verification-status"],
    () => {
      return axios.get("/email/status").then((response) => response.data);
    },
  );

  if (!data) {
    return null;
  }

  const { emailAddressUid, emailAddress, verifiedAt } = data;

  if (verifiedAt || !ENABLED) {
    return <Navigate to="/dashboard" />;
  }

  return (
    <Container className="pt-5 align-items-center text-center justify-content-center">
      <EnvelopeOpenFill className="mt-3" size={50} />
      <Title value="Verify Your Email" />

      <div className="px-3">
        <p>
          We've sent a verification email to <code>{emailAddress}</code>. You'll
          need to click the link inside to continue using the site.
        </p>
        <p className="lh-sm fw-light">
          <small>
            Can't see the email in your inbox? It might be worth checking your
            spam folder.
          </small>
        </p>
      </div>

      <h6 className="mt-5">Still Not There?</h6>

      <div className="px-3">
        <p className="lh-sm fw-light">
          <small>
            If you still don't see the email after a few minutes, you can{" "}
            <button
              className="text-primary"
              style={{
                backgroundColor: "transparent",
                border: "none",
                cursor: "pointer",
                textDecoration: "underline",
                display: "inline",
                margin: 0,
                padding: 0,
              }}
              onClick={() => axios.post(`/email/verify/resend`)}
            >
              resend it
            </button>{" "}
            or{" "}
            <button
              className="text-primary"
              style={{
                backgroundColor: "transparent",
                border: "none",
                cursor: "pointer",
                textDecoration: "underline",
                display: "inline",
                margin: 0,
                padding: 0,
              }}
            >
              change your email address
            </button>
            .
          </small>
        </p>
      </div>
    </Container>
  );
};

export default EmailVerification;
