import type { AnswerRequest } from "../types/AnswerRequest";
import type { NextResponse } from "../types/NextResponse";
import type { ReviewCard } from "../types/ReviewCard";

export type Rating = AnswerRequest["rating"];

export type ReviewState =
  | { status: "loading"; answered: number }
  | { status: "front"; card: ReviewCard; answered: number; draft: string }
  | { status: "back"; card: ReviewCard; answered: number; response?: string }
  | { status: "submitting"; card: ReviewCard; answered: number; response?: string }
  | { status: "done"; answered: number }
  | { status: "error"; message: string; answered: number };

export type ReviewEvent =
  | { type: "loaded"; response: NextResponse }
  | { type: "edit"; text: string }
  | { type: "flip" }
  | { type: "rate"; rating: Rating }
  | { type: "answered" }
  | { type: "failed"; message: string }
  | { type: "retry" };

export type KeyAction = { type: "none" } | { type: "flip" } | { type: "rate"; rating: Rating };

/** キー判定に必要な入力。DOM の KeyboardEvent から写す。 */
export type KeyEvent = {
  key: string;
  isComposing: boolean;
  keyCode: number;
  metaKey: boolean;
  ctrlKey: boolean;
  repeat: boolean;
  inTextarea: boolean;
};

const RATINGS: Record<string, Rating> = {
  "1": "again",
  "2": "hard",
  "3": "good",
  "4": "easy",
};

export const initialState: ReviewState = { status: "loading", answered: 0 };

export function isFreeText(card: ReviewCard): boolean {
  return card.level !== "beginner";
}

export function reduce(state: ReviewState, event: ReviewEvent): ReviewState {
  switch (event.type) {
    case "loaded":
      if (state.status !== "loading") {
        return state;
      }
      if (event.response.card === null) {
        return { status: "done", answered: state.answered };
      }
      return {
        status: "front",
        card: event.response.card,
        answered: state.answered,
        draft: "",
      };
    case "edit":
      if (state.status !== "front") {
        return state;
      }
      return { ...state, draft: event.text };
    case "flip":
      if (state.status !== "front") {
        return state;
      }
      if (!isFreeText(state.card)) {
        return { status: "back", card: state.card, answered: state.answered };
      }
      return {
        status: "back",
        card: state.card,
        answered: state.answered,
        response: state.draft,
      };
    case "rate":
      if (state.status !== "back") {
        return state;
      }
      if (state.response === undefined) {
        return { status: "submitting", card: state.card, answered: state.answered };
      }
      return {
        status: "submitting",
        card: state.card,
        answered: state.answered,
        response: state.response,
      };
    case "answered":
      if (state.status !== "submitting") {
        return state;
      }
      return { status: "loading", answered: state.answered + 1 };
    case "failed":
      return { status: "error", message: event.message, answered: state.answered };
    case "retry":
      if (state.status !== "error") {
        return state;
      }
      return { status: "loading", answered: state.answered };
  }
}

/** 自由記述で、空白以外の回答があるときだけ `response` を付ける。 */
export function answerRequest(state: ReviewState, rating: Rating): AnswerRequest | null {
  if (state.status !== "submitting") {
    return null;
  }
  const request: AnswerRequest = { stable_key: state.card.stable_key, rating };
  if (isFreeText(state.card) && state.response !== undefined && state.response.trim() !== "") {
    request.response = state.response;
  }
  return request;
}

export function keyAction(event: KeyEvent, state: ReviewState): KeyAction {
  if (event.repeat || event.isComposing || event.keyCode === 229) {
    return { type: "none" };
  }
  if (event.inTextarea) {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey) && state.status === "front") {
      return { type: "flip" };
    }
    return { type: "none" };
  }
  if (event.key === " " && state.status === "front" && !isFreeText(state.card)) {
    return { type: "flip" };
  }
  if (state.status === "back") {
    const rating = RATINGS[event.key];
    if (rating !== undefined) {
      return { type: "rate", rating };
    }
  }
  return { type: "none" };
}
