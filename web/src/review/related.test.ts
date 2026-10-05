import { describe, expect, it } from "vitest";
import type { ReviewCard } from "../types/ReviewCard";
import { isFreeText } from "./state";
import { relatedLabel } from "./related";

function card(level: string, ref_titles: string[]): ReviewCard {
  return {
    stable_key: "i1/integration/abc",
    note_id: "i1",
    title: "統合の入口",
    question: "どう組み合わせる？",
    answer: "照明が先。",
    level,
    rubric: null,
    ref_titles,
  };
}

describe("relatedLabel", () => {
  it("参照が無いと null", () => {
    expect(relatedLabel(card("integration", []))).toBeNull();
  });

  it("1 件はタイトルだけ", () => {
    expect(relatedLabel(card("integration", ["ゲイン側"]))).toBe("関連: ゲイン側");
  });

  it("2 件は参照順を / でつなぐ", () => {
    expect(relatedLabel(card("integration", ["ゲイン側", "照明側"]))).toBe(
      "関連: ゲイン側 / 照明側",
    );
  });
});

describe("isFreeText", () => {
  it("上級と統合は自由記述", () => {
    expect(isFreeText(card("advanced", []))).toBe(true);
    expect(isFreeText(card("integration", ["ゲイン側"]))).toBe(true);
    expect(isFreeText(card("beginner", []))).toBe(false);
  });
});
