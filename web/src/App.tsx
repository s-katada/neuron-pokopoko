import { useEffect, useRef, useState, type ReactNode } from "react";
import { initialState, reduce, type Rating, type ReviewState } from "./review/state";
import type { AnswerRequest } from "./types/AnswerRequest";
import type { NextResponse } from "./types/NextResponse";
import type { ReviewCard } from "./types/ReviewCard";

const RATINGS: { rating: Rating; label: string; key: string }[] = [
  { rating: "again", label: "もう一度", key: "1" },
  { rating: "hard", label: "難しい", key: "2" },
  { rating: "good", label: "普通", key: "3" },
  { rating: "easy", label: "簡単", key: "4" },
];

export default function App() {
  const [state, setState] = useState<ReviewState>(initialState);
  const stateRef = useRef(state);
  stateRef.current = state;

  useEffect(() => {
    if (state.status !== "loading") {
      return;
    }
    const controller = new AbortController();
    void fetch("/api/review/next", { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(String(response.status));
        }
        const body: unknown = await response.json();
        if (!isNextResponse(body)) {
          throw new Error("unexpected response");
        }
        setState((current) => reduce(current, { type: "loaded", response: body }));
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted) {
          return;
        }
        const message = error instanceof Error ? error.message : "error";
        setState((current) => reduce(current, { type: "failed", message }));
      });
    return () => {
      controller.abort();
    };
  }, [state.status, state.answered]);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.repeat) {
        return;
      }
      if (event.key === " ") {
        event.preventDefault();
        setState((current) => reduce(current, { type: "flip" }));
        return;
      }
      const rating = RATINGS.find((item) => item.key === event.key)?.rating;
      if (rating === undefined) {
        return;
      }
      event.preventDefault();
      rate(rating);
    }
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  function rate(rating: Rating) {
    const current = stateRef.current;
    if (current.status !== "back") {
      return;
    }
    const next = reduce(current, { type: "rate", rating });
    if (next === current) {
      return;
    }
    stateRef.current = next;
    setState(next);
    void sendAnswer(current.card, rating, setState, stateRef);
  }

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-lg flex-col justify-center gap-6 px-4 py-8">
      {state.status === "loading" && <p className="text-center text-lg">読み込み中</p>}
      {state.status === "front" && (
        <CardFace card={state.card}>
          <button
            type="button"
            className="min-h-14 w-full rounded-xl bg-neutral-900 text-lg text-white"
            onClick={() => {
              setState((current) => reduce(current, { type: "flip" }));
            }}
          >
            めくる
          </button>
        </CardFace>
      )}
      {state.status === "back" && (
        <CardFace card={state.card}>
          <p className="whitespace-pre-wrap text-lg">{state.card.answer}</p>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {RATINGS.map((item) => (
              <button
                key={item.rating}
                type="button"
                className="min-h-14 rounded-xl border border-neutral-300 text-base"
                onClick={() => {
                  rate(item.rating);
                }}
              >
                {item.label}
              </button>
            ))}
          </div>
        </CardFace>
      )}
      {state.status === "submitting" && (
        <CardFace card={state.card}>
          <p className="text-center text-lg">送信中</p>
        </CardFace>
      )}
      {state.status === "done" && (
        <div className="text-center">
          <p className="text-2xl font-semibold">今日の復習は完了</p>
          <p className="mt-2">{state.answered} 枚</p>
        </div>
      )}
      {state.status === "error" && (
        <div className="text-center">
          <p>{state.message}</p>
          <button
            type="button"
            className="mt-4 min-h-14 rounded-xl bg-neutral-900 px-6 text-lg text-white"
            onClick={() => {
              setState((current) => reduce(current, { type: "retry" }));
            }}
          >
            再読み込み
          </button>
        </div>
      )}
    </main>
  );
}

function CardFace({ card, children }: { card: ReviewCard; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-6">
      <p className="text-sm text-neutral-500">{card.title}</p>
      <h1 className="text-2xl font-semibold">{card.question}</h1>
      {children}
    </section>
  );
}

async function sendAnswer(
  card: ReviewCard,
  rating: Rating,
  setState: (update: (current: ReviewState) => ReviewState) => void,
  stateRef: { current: ReviewState },
) {
  try {
    const payload: AnswerRequest = { stable_key: card.stable_key, rating };
    const response = await fetch("/api/review/answer", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
    });
    if (!response.ok) {
      throw new Error(String(response.status));
    }
    const next = (current: ReviewState) => reduce(current, { type: "answered" });
    stateRef.current = next(stateRef.current);
    setState(next);
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : "error";
    const next = (current: ReviewState) => reduce(current, { type: "failed", message });
    stateRef.current = next(stateRef.current);
    setState(next);
  }
}

function isNextResponse(body: unknown): body is NextResponse {
  if (typeof body !== "object" || body === null || !("card" in body)) {
    return false;
  }
  const card = body.card;
  if (card === null) {
    return true;
  }
  if (typeof card !== "object" || card === null) {
    return false;
  }
  const row = card as Record<string, unknown>;
  return (
    typeof row.stable_key === "string" &&
    typeof row.note_id === "string" &&
    typeof row.title === "string" &&
    typeof row.question === "string" &&
    typeof row.answer === "string" &&
    typeof row.level === "string"
  );
}
