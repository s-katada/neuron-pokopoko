import { useEffect, useState } from "react";
import { barHeights, formatDay, formatPercent } from "./format";
import type { DailyCount } from "../types/DailyCount";

export default function StatsPage() {
  const [overall, setOverall] = useState<number | null | undefined>(undefined);
  const [days, setDays] = useState<DailyCount[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    void loadStats(controller.signal)
      .then((loaded) => {
        setOverall(loaded.overall);
        setDays(loaded.days);
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
  if (days === null || overall === undefined) {
    return <p>読み込み中</p>;
  }

  const heights = barHeights(days.map((day) => day.count));

  return (
    <div className="flex flex-col gap-8">
      <p className="text-5xl font-semibold tabular-nums">{formatPercent(overall)}</p>
      <section className="flex flex-col gap-3">
        <h1 className="text-sm text-neutral-500">直近 {days.length} 日</h1>
        <div className="flex items-end gap-px">
          {days.map((day, index) => (
            <DayBar key={day.day_start} day={day} height={heights[index] ?? 0} />
          ))}
        </div>
      </section>
    </div>
  );
}

function DayBar({ day, height }: { day: DailyCount; height: number }) {
  const label = formatDay(day.day_start);
  return (
    <div
      className="flex min-w-0 flex-1 flex-col items-center gap-1"
      title={`${label} ${day.count}`}
    >
      <span className="text-[8px] leading-none tabular-nums text-neutral-500">{day.count}</span>
      <div className="relative h-28 w-full">
        <div
          className="absolute inset-x-0 bottom-0 rounded-sm bg-neutral-800"
          style={{ height: `${height}%` }}
        />
      </div>
      <span className="text-[8px] leading-none [writing-mode:vertical-rl]">{label}</span>
    </div>
  );
}

async function loadStats(
  signal: AbortSignal,
): Promise<{ overall: number | null; days: DailyCount[] }> {
  const [treeResponse, dailyResponse] = await Promise.all([
    fetch("/api/tree", { signal }),
    fetch("/api/stats/daily", { signal }),
  ]);
  if (!treeResponse.ok || !dailyResponse.ok) {
    throw new Error("request failed");
  }
  const tree: unknown = await treeResponse.json();
  const daily: unknown = await dailyResponse.json();
  if (!isOverall(tree) || !isDaily(daily)) {
    throw new Error("unexpected response");
  }
  return { overall: tree.overall, days: daily };
}

function isOverall(body: unknown): body is { overall: number | null } {
  if (typeof body !== "object" || body === null || !("overall" in body)) {
    return false;
  }
  const overall = body.overall;
  return overall === null || typeof overall === "number";
}

function isDaily(body: unknown): body is DailyCount[] {
  return Array.isArray(body) && body.every(isDailyCount);
}

function isDailyCount(body: unknown): body is DailyCount {
  if (typeof body !== "object" || body === null) {
    return false;
  }
  const day = body as { day_start?: unknown; count?: unknown };
  return typeof day.day_start === "number" && typeof day.count === "number";
}
