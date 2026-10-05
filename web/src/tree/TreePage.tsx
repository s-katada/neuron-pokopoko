import { useEffect, useState } from "react";
import { apiFetch } from "../api";
import { Link } from "react-router";
import { formatPercent } from "../stats/format";
import type { MajorStats } from "../types/MajorStats";
import type { MiddleStats } from "../types/MiddleStats";
import type { MinorStats } from "../types/MinorStats";
import type { NoteStats } from "../types/NoteStats";
import type { Tree } from "../types/Tree";

export default function TreePage() {
  const [tree, setTree] = useState<Tree | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    void apiFetch("/api/tree", { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(String(response.status));
        }
        const body: unknown = await response.json();
        if (!isTree(body)) {
          throw new Error("unexpected response");
        }
        setTree(body);
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
  }, []);

  if (error !== null) {
    return <p>読み込めませんでした</p>;
  }
  if (tree === null) {
    return <p>読み込み中</p>;
  }

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-neutral-500">全体 {formatPercent(tree.overall)}</p>
      {tree.majors.map((major) => (
        <MajorNode key={major.major} major={major} />
      ))}
    </div>
  );
}

function MajorNode({ major }: { major: MajorStats }) {
  return (
    <details className="rounded-xl border border-neutral-200 px-3 py-1">
      <Summary label={major.major} retention={major.retention} cardCount={major.card_count} />
      <div className="flex flex-col gap-2 pb-2 pl-3">
        {major.middles.map((middle) => (
          <MiddleNode key={middle.middle} middle={middle} />
        ))}
      </div>
    </details>
  );
}

function MiddleNode({ middle }: { middle: MiddleStats }) {
  return (
    <details>
      <Summary label={middle.middle} retention={middle.retention} cardCount={middle.card_count} />
      <div className="flex flex-col gap-2 pb-2 pl-3">
        {middle.minors.map((minor) => (
          <MinorNode key={minor.minor} minor={minor} />
        ))}
      </div>
    </details>
  );
}

function MinorNode({ minor }: { minor: MinorStats }) {
  return (
    <details>
      <Summary label={minor.minor} retention={minor.retention} cardCount={minor.card_count} />
      <ul className="flex flex-col gap-1 pb-2 pl-3">
        {minor.notes.map((note) => (
          <NoteLink key={note.note_id} note={note} />
        ))}
      </ul>
    </details>
  );
}

function NoteLink({ note }: { note: NoteStats }) {
  return (
    <li className="flex items-baseline justify-between gap-3">
      <Link to={`/notes/${note.note_id}`} className="break-words underline">
        {note.title}
      </Link>
      <Stat retention={note.retention} cardCount={note.card_count} />
    </li>
  );
}

function Summary({
  label,
  retention,
  cardCount,
}: {
  label: string;
  retention: number | null;
  cardCount: number;
}) {
  return (
    <summary className="cursor-pointer py-2">
      <span className="font-medium break-words">{label}</span>
      <span className="ml-3">
        <Stat retention={retention} cardCount={cardCount} />
      </span>
    </summary>
  );
}

function Stat({ retention, cardCount }: { retention: number | null; cardCount: number }) {
  return (
    <span className="shrink-0 text-sm text-neutral-500">
      {formatPercent(retention)} · {cardCount}枚
    </span>
  );
}

function isTree(body: unknown): body is Tree {
  return (
    typeof body === "object" &&
    body !== null &&
    "overall" in body &&
    "majors" in body &&
    Array.isArray(body.majors)
  );
}
