import Button from "react-bootstrap/Button";
import Modal from "react-bootstrap/Modal";

interface Props {
  show: boolean;
  heading: string;
  body: string;
  handleConfirmation: () => void;
  closeModal: () => void;
}

const ConfirmationModal = ({
  show,
  heading,
  body,
  handleConfirmation,
  closeModal,
}: Props) => {
  const handleSubmit = () => {
    // Perform the action, then close the modal
    handleConfirmation();
    closeModal();
  };

  return (
    <Modal show={show} onHide={closeModal}>
      <Modal.Header closeButton>
        <Modal.Title>{heading}</Modal.Title>
      </Modal.Header>

      <Modal.Body>{body}</Modal.Body>

      <Modal.Footer>
        <Button
          variant="danger"
          onClick={handleSubmit}
          aria-label="confirm-pending-action"
        >
          Confirm
        </Button>
        <Button
          variant="primary"
          onClick={closeModal}
          aria-label="cancel-pending-action"
        >
          Cancel
        </Button>
      </Modal.Footer>
    </Modal>
  );
};

export default ConfirmationModal;
