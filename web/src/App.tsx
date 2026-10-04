import { useEffect, useState } from "react";
import type { DemoSchedule } from "./types/DemoSchedule";

type Health = { status: "loading" } | { status: "ok" } | { status: "error"; message: string };

type ScheduleView =
  | { status: "loading" }
  | { status: "ok"; schedule: DemoSchedule }
  | { status: "error"; message: string };

export default function App() {
  const [health, setHealth] = useState<Health>({ status: "loading" });
  const [schedule, setSchedule] = useState<ScheduleView>({ status: "loading" });

  useEffect(() => {
    const controller = new AbortController();

    void fetch("/api/health", { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(String(response.status));
        }
        const body: unknown = await response.json();
        if (!isOk(body)) {
          throw new Error("unexpected response");
        }
        setHealth({ status: "ok" });
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted) {
          return;
        }
        const message = error instanceof Error ? error.message : "error";
        setHealth({ status: "error", message });
      });

    return () => {
      controller.abort();
    };
  }, []);

  useEffect(() => {
    const controller = new AbortController();

    void fetch("/api/demo-schedule", { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(String(response.status));
        }
        const body: unknown = await response.json();
        if (!isDemoSchedule(body)) {
          throw new Error("unexpected response");
        }
        setSchedule({ status: "ok", schedule: body });
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted) {
          return;
        }
        const message = error instanceof Error ? error.message : "error";
        setSchedule({ status: "error", message });
      });

    return () => {
      controller.abort();
    };
  }, []);

  return (
    <main className="grid min-h-screen place-items-center">
      <div className="text-center">
        <h1 className="text-2xl font-semibold">ニューロンポコポコ</h1>
        <p className="mt-2">
          {health.status === "loading" && "確認中"}
          {health.status === "ok" && "ok"}
          {health.status === "error" && `エラー: ${health.message}`}
        </p>
        <p className="mt-2">
          {schedule.status === "loading" && "間隔を計算中"}
          {schedule.status === "ok" && `間隔: ${schedule.schedule.interval_days}`}
          {schedule.status === "error" && `間隔のエラー: ${schedule.message}`}
        </p>
      </div>
    </main>
  );
}

function isOk(body: unknown): boolean {
  return typeof body === "object" && body !== null && "ok" in body && body.ok === true;
}

function isDemoSchedule(body: unknown): body is DemoSchedule {
  if (typeof body !== "object" || body === null) {
    return false;
  }
  const row = body as Record<string, unknown>;
  return (
    typeof row.rating === "string" &&
    typeof row.interval_days === "number" &&
    typeof row.stability === "number" &&
    typeof row.difficulty === "number"
  );
}
