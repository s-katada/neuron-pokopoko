import { describe, expect, test } from "vitest";
import { formatJst, formatPercent } from "./format";

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

describe("formatJst", () => {
  test("formats a unix second in Asia/Tokyo", () => {
    expect(formatJst(1_791_158_400)).toBe("2026/10/05 09:00");
  });
});
