import { describe, expect, it } from "vitest";
import type { ReviewCard } from "../types/ReviewCard";
import { type KeyEvent, type ReviewState, answerRequest, keyAction, reduce } from "./state";

const card: ReviewCard = {
  stable_key: "gain/beginner/34a0dbf6827d4910",
  note_id: "gain",
  title: "ゲイン",
  question: "ゲインとは？",
  answer: "撮像素子の出力を増幅する倍率。",
  level: "beginner",
  rubric: null,
};

const mid: ReviewCard = {
  stable_key: "gain/intermediate/d090164090665435",
  note_id: "gain",
  title: "ゲイン",
  question: "なぜゲインを上げるとノイズが目立つ？",
  answer: "受光量は増えず、信号とノイズをまとめて増幅するから。",
  level: "intermediate",
  rubric: "「受光量は増えない」「S/N 比は改善しない」に触れている",
};

const front: ReviewState = { status: "front", card, answered: 0, draft: "" };

function keys(partial: Partial<KeyEvent>): KeyEvent {
  return {
    key: "",
    isComposing: false,
    keyCode: 0,
    metaKey: false,
    ctrlKey: false,
    repeat: false,
    inTextarea: false,
    ...partial,
  };
}

describe("reduce", () => {
  it("edit は front だけ下書きを変える", () => {
    expect(reduce(front, { type: "edit", text: "増幅" })).toEqual({
      status: "front",
      card,
      answered: 0,
      draft: "増幅",
    });
    const back: ReviewState = { status: "back", card, answered: 0 };
    const submitting: ReviewState = { status: "submitting", card, answered: 0 };
    expect(reduce(back, { type: "edit", text: "増幅" })).toBe(back);
    expect(reduce(submitting, { type: "edit", text: "増幅" })).toBe(submitting);
  });

  it("自由記述の flip は draft を response に入れる", () => {
    const writing: ReviewState = {
      status: "front",
      card: mid,
      answered: 1,
      draft: "受光量はそのまま",
    };
    expect(reduce(writing, { type: "flip" })).toEqual({
      status: "back",
      card: mid,
      answered: 1,
      response: "受光量はそのまま",
    });
  });

  it("初級の flip は response を持たない", () => {
    const writing: ReviewState = { status: "front", card, answered: 0, draft: "これは送らない" };
    expect(reduce(writing, { type: "flip" })).toEqual({ status: "back", card, answered: 0 });
  });

  it("rate は自由記述の response を submitting に載せ、空なら送らない", () => {
    const back: ReviewState = {
      status: "back",
      card: mid,
      answered: 2,
      response: "受光量は増えない",
    };
    const submitting = reduce(back, { type: "rate", rating: "good" });
    expect(submitting).toEqual({
      status: "submitting",
      card: mid,
      answered: 2,
      response: "受光量は増えない",
    });
    expect(answerRequest(submitting, "good")).toEqual({
      stable_key: mid.stable_key,
      rating: "good",
      response: "受光量は増えない",
    });

    const blank = reduce(
      { status: "back", card: mid, answered: 0, response: "  " },
      { type: "rate", rating: "hard" },
    );
    expect(answerRequest(blank, "hard")).toEqual({
      stable_key: mid.stable_key,
      rating: "hard",
    });
  });

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
    const withResponse: ReviewState = {
      status: "submitting",
      card: mid,
      answered: 0,
      response: "書いた",
    };
    expect(reduce(withResponse, { type: "rate", rating: "again" })).toBe(withResponse);
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

describe("keyAction", () => {
  const writing: ReviewState = { status: "front", card: mid, answered: 0, draft: "" };
  const back: ReviewState = { status: "back", card: mid, answered: 0, response: "答え" };

  it("IME 変換中は無視する", () => {
    expect(
      keyAction(
        keys({ key: "Enter", isComposing: true, metaKey: true, inTextarea: true }),
        writing,
      ),
    ).toEqual({
      type: "none",
    });
    expect(keyAction(keys({ key: " ", keyCode: 229 }), front)).toEqual({ type: "none" });
    expect(keyAction(keys({ key: "1", keyCode: 229 }), back)).toEqual({ type: "none" });
  });

  it("textarea のスペースと数字は無視する", () => {
    expect(keyAction(keys({ key: " ", inTextarea: true }), writing)).toEqual({ type: "none" });
    for (const key of ["1", "2", "3", "4"]) {
      expect(keyAction(keys({ key, inTextarea: true }), writing)).toEqual({ type: "none" });
      expect(keyAction(keys({ key, inTextarea: true }), back)).toEqual({ type: "none" });
    }
  });

  it("textarea の Cmd+Enter と Ctrl+Enter は flip", () => {
    expect(keyAction(keys({ key: "Enter", metaKey: true, inTextarea: true }), writing)).toEqual({
      type: "flip",
    });
    expect(keyAction(keys({ key: "Enter", ctrlKey: true, inTextarea: true }), writing)).toEqual({
      type: "flip",
    });
  });

  it("初級 front のスペースは flip", () => {
    expect(keyAction(keys({ key: " " }), front)).toEqual({ type: "flip" });
    expect(keyAction(keys({ key: " " }), writing)).toEqual({ type: "none" });
  });

  it("back の 1〜4 は rate", () => {
    expect(keyAction(keys({ key: "1" }), back)).toEqual({ type: "rate", rating: "again" });
    expect(keyAction(keys({ key: "2" }), back)).toEqual({ type: "rate", rating: "hard" });
    expect(keyAction(keys({ key: "3" }), back)).toEqual({ type: "rate", rating: "good" });
    expect(keyAction(keys({ key: "4" }), back)).toEqual({ type: "rate", rating: "easy" });
  });
});
