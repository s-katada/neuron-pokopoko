import { describe, expect, it } from "vitest";
import type { ReviewCard } from "../types/ReviewCard";
import { type ReviewState, reduce } from "./state";

const card: ReviewCard = {
  stable_key: "gain/beginner/34a0dbf6827d4910",
  note_id: "gain",
  title: "ゲイン",
  question: "ゲインとは？",
  answer: "撮像素子の出力を増幅する倍率。",
  level: "beginner",
};

const front: ReviewState = { status: "front", card, answered: 0 };

describe("reduce", () => {
  it("front で flip すると back になる", () => {
    expect(reduce(front, { type: "flip" })).toEqual({ status: "back", card, answered: 0 });
  });

  it("back 以外の flip は無視する", () => {
    const back: ReviewState = { status: "back", card, answered: 0 };
    const submitting: ReviewState = { status: "submitting", card, answered: 0 };
    const loading: ReviewState = { status: "loading", answered: 1 };
    const done: ReviewState = { status: "done", answered: 1 };
    const error: ReviewState = { status: "error", message: "落ちた", answered: 1 };
    expect(reduce(back, { type: "flip" })).toBe(back);
    expect(reduce(submitting, { type: "flip" })).toBe(submitting);
    expect(reduce(loading, { type: "flip" })).toBe(loading);
    expect(reduce(done, { type: "flip" })).toBe(done);
    expect(reduce(error, { type: "flip" })).toBe(error);
  });

  it("back で rate すると submitting になる", () => {
    const back: ReviewState = { status: "back", card, answered: 2 };
    expect(reduce(back, { type: "rate", rating: "good" })).toEqual({
      status: "submitting",
      card,
      answered: 2,
    });
  });

  it("submitting 中の rate は無視する", () => {
    const submitting: ReviewState = { status: "submitting", card, answered: 0 };
    expect(reduce(submitting, { type: "rate", rating: "easy" })).toBe(submitting);
  });

  it("loaded の card が null なら done になる", () => {
    const loading: ReviewState = { status: "loading", answered: 3 };
    expect(reduce(loading, { type: "loaded", response: { card: null } })).toEqual({
      status: "done",
      answered: 3,
    });
  });

  it("failed のあと retry で loading に戻る", () => {
    const loading: ReviewState = { status: "loading", answered: 1 };
    const error = reduce(loading, { type: "failed", message: "500" });
    expect(error).toEqual({ status: "error", message: "500", answered: 1 });
    expect(reduce(error, { type: "retry" })).toEqual({ status: "loading", answered: 1 });
  });
});
