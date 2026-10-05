import { describe, expect, test } from "vitest";
import { barHeights, formatDay, formatJst, formatPercent } from "./format";

describe("formatPercent", () => {
  test("rounds to an integer percent", () => {
    expect(formatPercent(0.925)).toBe("93%");
    expect(formatPercent(0.9)).toBe("90%");
    expect(formatPercent(1)).toBe("100%");
  });

  test("renders a dash when retention is missing", () => {
    expect(formatPercent(null)).toBe("—");
  });
});

describe("barHeights", () => {
  test("scales to the largest count", () => {
    expect(barHeights([0, 5, 10])).toEqual([0, 50, 100]);
  });

  test("stays at zero when every day is empty", () => {
    expect(barHeights([0, 0, 0])).toEqual([0, 0, 0]);
  });

  test("returns an empty list for no days", () => {
    expect(barHeights([])).toEqual([]);
  });
});

describe("formatDay", () => {
  test("formats a study-day start as month/day in Asia/Tokyo", () => {
    expect(formatDay(1_791_140_400)).toBe("10/5");
  });
});

describe("formatJst", () => {
  test("formats a unix second in Asia/Tokyo", () => {
    expect(formatJst(1_791_158_400)).toBe("2026/10/05 09:00");
  });
});
