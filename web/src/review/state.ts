import type { AnswerRequest } from "../types/AnswerRequest";
import type { NextResponse } from "../types/NextResponse";
import type { ReviewCard } from "../types/ReviewCard";

export type Rating = AnswerRequest["rating"];

export type ReviewState =
  | { status: "loading"; answered: number }
  | { status: "front"; card: ReviewCard; answered: number }
  | { status: "back"; card: ReviewCard; answered: number }
  | { status: "submitting"; card: ReviewCard; answered: number }
  | { status: "done"; answered: number }
  | { status: "error"; message: string; answered: number };

export type ReviewEvent =
  | { type: "loaded"; response: NextResponse }
  | { type: "flip" }
  | { type: "rate"; rating: Rating }
  | { type: "answered" }
  | { type: "failed"; message: string }
  | { type: "retry" };

export const initialState: ReviewState = { status: "loading", answered: 0 };

export function reduce(state: ReviewState, event: ReviewEvent): ReviewState {
  switch (event.type) {
    case "loaded":
      if (state.status !== "loading") {
        return state;
      }
      if (event.response.card === null) {
        return { status: "done", answered: state.answered };
      }
      return { status: "front", card: event.response.card, answered: state.answered };
    case "flip":
      if (state.status !== "front") {
        return state;
      }
      return { status: "back", card: state.card, answered: state.answered };
    case "rate":
      if (state.status !== "back") {
        return state;
      }
      return { status: "submitting", card: state.card, answered: state.answered };
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
