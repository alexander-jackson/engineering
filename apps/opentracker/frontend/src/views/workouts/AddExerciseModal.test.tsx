import { formatDate } from "./AddExerciseModal";

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
