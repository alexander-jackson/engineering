import { useEffect } from "react";
import axios from "axios";
import { useParams } from "react-router-dom";

const VerifyEmail = () => {
  const { id } = useParams();

  useEffect(() => {
    axios.put(`/email/verify/${id}`);
  }, [id]);

  return <p>Hello World (id={id})</p>;
};

export default VerifyEmail;
