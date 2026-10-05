import { useEffect, useRef, useState, type ReactNode } from "react";
import { levelBadgeClass, levelLabel } from "./level";
import { relatedLabel } from "./related";
import {
  answerRequest,
  initialState,
  isFreeText,
  keyAction,
  reduce,
  type Rating,
  type ReviewState,
} from "./state";
import type { AnswerRequest } from "../types/AnswerRequest";
import type { NextResponse } from "../types/NextResponse";
import type { ReviewCard } from "../types/ReviewCard";

const RATINGS: { rating: Rating; label: string; key: string }[] = [
  { rating: "again", label: "もう一度", key: "1" },
  { rating: "hard", label: "難しい", key: "2" },
  { rating: "good", label: "普通", key: "3" },
  { rating: "easy", label: "簡単", key: "4" },
];

export default function ReviewPage() {
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
      const action = keyAction(
        {
          key: event.key,
          isComposing: event.isComposing,
          keyCode: event.keyCode,
          metaKey: event.metaKey,
          ctrlKey: event.ctrlKey,
          repeat: event.repeat,
          inTextarea: event.target instanceof HTMLTextAreaElement,
        },
        stateRef.current,
      );
      if (action.type === "none") {
        return;
      }
      event.preventDefault();
      if (action.type === "flip") {
        setState((current) => reduce(current, { type: "flip" }));
        return;
      }
      rate(action.rating);
    }
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  function rate(rating: Rating) {
    const current = stateRef.current;
    const next = reduce(current, { type: "rate", rating });
    if (next === current || next.status !== "submitting") {
      return;
    }
    const payload = answerRequest(next, rating);
    if (payload === null) {
      return;
    }
    stateRef.current = next;
    setState(next);
    void sendAnswer(payload, setState, stateRef);
  }

  return (
    <div className="flex flex-col gap-6">
      {state.status === "loading" && <p className="text-center text-lg">読み込み中</p>}
      {state.status === "front" && isFreeText(state.card) && (
        <CardFace card={state.card}>
          <textarea
            value={state.draft}
            rows={5}
            aria-label="自分の答え"
            className="w-full resize-y rounded-xl border border-neutral-300 p-3 text-base leading-relaxed"
            onChange={(event) => {
              const text = event.target.value;
              setState((current) => reduce(current, { type: "edit", text }));
            }}
          />
          <button
            type="button"
            className="min-h-14 w-full rounded-xl bg-neutral-900 text-lg text-white"
            onClick={() => {
              setState((current) => reduce(current, { type: "flip" }));
            }}
          >
            答え合わせ
          </button>
        </CardFace>
      )}
      {state.status === "front" && !isFreeText(state.card) && (
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
          {isFreeText(state.card) ? (
            <FreeTextReveal card={state.card} response={state.response} />
          ) : (
            <p className="whitespace-pre-wrap text-lg leading-relaxed break-words">
              {state.card.answer}
            </p>
          )}
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
    </div>
  );
}

function CardFace({ card, children }: { card: ReviewCard; children: ReactNode }) {
  const related = card.level === "integration" ? relatedLabel(card) : null;
  return (
    <section className="flex flex-col gap-6">
      <p className="flex items-center gap-2 text-sm text-neutral-500">
        <span>{card.title}</span>
        <span className={`rounded-full px-2 py-0.5 text-xs ${levelBadgeClass(card.level)}`}>
          {levelLabel(card.level)}
        </span>
      </p>
      <div className="flex flex-col gap-2">
        {related !== null && (
          <p className="text-sm leading-relaxed break-words text-neutral-600">{related}</p>
        )}
        <h1 className="text-xl font-semibold break-words sm:text-2xl">{card.question}</h1>
      </div>
      {children}
    </section>
  );
}

function FreeTextReveal({ card, response }: { card: ReviewCard; response: string | undefined }) {
  const own = response !== undefined && response.trim() !== "" ? response : "(未記入)";
  return (
    <div className="flex flex-col gap-4 text-base leading-relaxed">
      <div>
        <h2 className="text-sm font-medium text-neutral-500">自分の答え</h2>
        <p className="mt-1 whitespace-pre-wrap break-words">{own}</p>
      </div>
      <div>
        <h2 className="text-sm font-medium text-neutral-500">模範解答</h2>
        <p className="mt-1 whitespace-pre-wrap break-words">{card.answer}</p>
      </div>
      {card.rubric !== null && card.rubric !== "" && (
        <div>
          <h2 className="text-sm font-medium text-neutral-500">採点基準</h2>
          <p className="mt-1 whitespace-pre-wrap break-words">{card.rubric}</p>
        </div>
      )}
    </div>
  );
}

async function sendAnswer(
  payload: AnswerRequest,
  setState: (update: (current: ReviewState) => ReviewState) => void,
  stateRef: { current: ReviewState },
) {
  try {
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
