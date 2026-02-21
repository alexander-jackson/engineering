import "@testing-library/jest-dom";
import { render, screen, fireEvent } from "@testing-library/react";
import AddExerciseModal, { formatDate } from "./AddExerciseModal";
import { ExerciseVariant, Exercise } from "~/shared/types";
import { useUniqueExercises } from "~/hooks/useAnalysis";
import { useLastSession } from "~/hooks/useLastSession";

jest.mock("~/hooks/useAnalysis");
jest.mock("~/hooks/useLastSession");

describe("formatDate", () => {
  it("formats date with 1st ordinal", () => {
    expect(formatDate("2024-01-01")).toBe("Mon 1st January");
  });

  it("formats date with 2nd ordinal", () => {
    expect(formatDate("2024-01-02")).toBe("Tue 2nd January");
  });

  it("formats date with 3rd ordinal", () => {
    expect(formatDate("2024-01-03")).toBe("Wed 3rd January");
  });

  it("formats date with 4th-20th ordinals (th)", () => {
    expect(formatDate("2024-01-04")).toBe("Thu 4th January");
    expect(formatDate("2024-01-11")).toBe("Thu 11th January");
    expect(formatDate("2024-01-12")).toBe("Fri 12th January");
    expect(formatDate("2024-01-13")).toBe("Sat 13th January");
    expect(formatDate("2024-01-20")).toBe("Sat 20th January");
  });

  it("formats date with 21st, 22nd, 23rd ordinals", () => {
    expect(formatDate("2024-01-21")).toBe("Sun 21st January");
    expect(formatDate("2024-01-22")).toBe("Mon 22nd January");
    expect(formatDate("2024-01-23")).toBe("Tue 23rd January");
  });

  it("formats date with 31st ordinal", () => {
    expect(formatDate("2024-01-31")).toBe("Wed 31st January");
  });

  it("formats different months correctly", () => {
    expect(formatDate("2024-02-15")).toBe("Thu 15th February");
    expect(formatDate("2024-03-10")).toBe("Sun 10th March");
    expect(formatDate("2024-12-25")).toBe("Wed 25th December");
  });

  it("handles leap year dates", () => {
    expect(formatDate("2024-02-29")).toBe("Thu 29th February");
  });
});

describe("AddExerciseModal", () => {
  const mockUseUniqueExercises = useUniqueExercises as jest.Mock;
  const mockUseLastSession = useLastSession as jest.Mock;

  const latPulldown: Exercise = {
    variant: ExerciseVariant.Other,
    description: "Lat Pulldown",
    weight: 80,
    reps: 10,
    sets: 3,
  };

  const renderModal = (exercises: Exercise[] = [latPulldown]) =>
    render(
      <AddExerciseModal
        show={true}
        onHide={jest.fn()}
        exercises={exercises}
        setExercises={jest.fn()}
        currentDate="2024-01-15"
        saveWorkout={jest.fn()}
      />,
    );

  beforeEach(() => {
    mockUseUniqueExercises.mockReturnValue({ data: ["Lat Pulldown", "Cable Row"] });
    mockUseLastSession.mockReturnValue({
      data: {
        recorded: "2024-01-10",
        exercise: latPulldown,
      },
      isLoading: false,
    });
  });

  it("uses the resolved variant and description to query the previous session", () => {
    renderModal();

    expect(mockUseLastSession).toHaveBeenCalledWith(
      ExerciseVariant.Other,
      "Lat Pulldown",
      "2024-01-15",
    );
  });

  it("uses the resolved variant to fetch unique exercises for suggestions", () => {
    renderModal();

    expect(mockUseUniqueExercises).toHaveBeenCalledWith(ExerciseVariant.Other);
  });

  it("displays the previous session information from the last workout", () => {
    renderModal();

    expect(
      screen.getByText("Previous Session (Wed 10th January)"),
    ).toBeInTheDocument();
    expect(screen.getByText("80 kg")).toBeInTheDocument();
    expect(screen.getByText("10")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("shows matching suggestions as the user types in the description field", () => {
    renderModal();

    fireEvent.change(screen.getByLabelText("Exercise"), {
      target: { value: "Cable" },
    });

    expect(screen.getByRole("button", { name: "Cable Row" })).toBeInTheDocument();
  });

  it("does not query previous session or suggestions without a placeholder", () => {
    renderModal([]);

    expect(mockUseLastSession).toHaveBeenCalledWith(
      undefined,
      undefined,
      "2024-01-15",
    );
    expect(mockUseUniqueExercises).toHaveBeenCalledWith(ExerciseVariant.Unknown);
  });
});
