import { useEffect, useState } from "react";
import { apiFetch } from "../api";
import { Link, useParams } from "react-router";
import { levelBadgeClass, levelLabel } from "../review/level";
import { formatJst, formatPercent } from "../stats/format";
import type { DetailCard } from "../types/DetailCard";
import type { NoteDetail } from "../types/NoteDetail";
import type { Tree } from "../types/Tree";

export default function NotePage() {
  const { id } = useParams();
  const [detail, setDetail] = useState<NoteDetail | null>(null);
  const [place, setPlace] = useState<string | null>(null);
  const [missing, setMissing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (id === undefined) {
      return;
    }
    const controller = new AbortController();
    setDetail(null);
    setPlace(null);
    setMissing(false);
    setError(null);
    void loadNote(id, controller.signal)
      .then((loaded) => {
        if (loaded.kind === "missing") {
          setMissing(true);
          return;
        }
        setDetail(loaded.detail);
        setPlace(loaded.place);
      })
      .catch((caught: unknown) => {
        if (controller.signal.aborted) {
          return;
        }
        setError(caught instanceof Error ? caught.message : "error");
      });
    return () => {
      controller.abort();
    };
  }, [id]);

  if (missing) {
    return (
      <div className="flex flex-col gap-4">
        <p>ノートが見つかりません</p>
        <Link to="/tree" className="underline">
          ツリー
        </Link>
      </div>
    );
  }
  if (error !== null) {
    return <p>読み込めませんでした</p>;
  }
  if (detail === null) {
    return <p>読み込み中</p>;
  }

  return (
    <div className="flex flex-col gap-6">
      <header>
        <h1 className="text-xl font-semibold break-words">{detail.title}</h1>
        <p className="mt-1 text-sm text-neutral-500">{place ?? "—"}</p>
      </header>
      <ul className="flex flex-col gap-4">
        {detail.cards.map((card) => (
          <CardRow key={`${card.level}:${card.question}`} card={card} />
        ))}
      </ul>
    </div>
  );
}

function CardRow({ card }: { card: DetailCard }) {
  return (
    <li className="flex flex-col gap-2 border-t border-neutral-200 pt-4">
      <p>
        <span className={`rounded-full px-2 py-0.5 text-xs ${levelBadgeClass(card.level)}`}>
          {levelLabel(card.level)}
        </span>
      </p>
      <p className="break-words">{card.question}</p>
      {card.fsrs_state === "new" ? (
        <p className="text-sm text-neutral-500">未学習</p>
      ) : (
        <p className="text-sm text-neutral-500">
          次回: {card.due_at === null ? "—" : formatJst(card.due_at)} ·{" "}
          {formatPercent(card.retention)}
        </p>
      )}
    </li>
  );
}

async function loadNote(
  id: string,
  signal: AbortSignal,
): Promise<{ kind: "missing" } | { kind: "ok"; detail: NoteDetail; place: string | null }> {
  const [noteResponse, treeResponse] = await Promise.all([
    apiFetch(`/api/notes/${id}`, { signal }),
    apiFetch("/api/tree", { signal }),
  ]);
  if (noteResponse.status === 404) {
    return { kind: "missing" };
  }
  if (!noteResponse.ok) {
    throw new Error(String(noteResponse.status));
  }
  const body: unknown = await noteResponse.json();
  if (!isNoteDetail(body)) {
    throw new Error("unexpected response");
  }
  const place = treeResponse.ok ? placeFromTree(await treeResponse.json(), id) : null;
  return { kind: "ok", detail: body, place };
}

function placeFromTree(body: unknown, noteId: string): string | null {
  if (!isTree(body)) {
    return null;
  }
  for (const major of body.majors) {
    for (const middle of major.middles) {
      for (const minor of middle.minors) {
        if (minor.notes.some((note) => note.note_id === noteId)) {
          return `${major.major} / ${middle.middle} / ${minor.minor}`;
        }
      }
    }
  }
  return null;
}

function isNoteDetail(body: unknown): body is NoteDetail {
  if (typeof body !== "object" || body === null) {
    return false;
  }
  const row = body as Record<string, unknown>;
  return (
    typeof row.note_id === "string" && typeof row.title === "string" && Array.isArray(row.cards)
  );
}

function isTree(body: unknown): body is Tree {
  return (
    typeof body === "object" && body !== null && "majors" in body && Array.isArray(body.majors)
  );
}
