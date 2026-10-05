import { useEffect, useState } from "react";

type Health = { status: "loading" } | { status: "ok" } | { status: "error"; message: string };

export default function App() {
  const [health, setHealth] = useState<Health>({ status: "loading" });

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

  return (
    <main className="grid min-h-screen place-items-center">
      <div className="text-center">
        <h1 className="text-2xl font-semibold">ニューロンポコポコ</h1>
        <p className="mt-2">
          {health.status === "loading" && "確認中"}
          {health.status === "ok" && "ok"}
          {health.status === "error" && `エラー: ${health.message}`}
        </p>
      </div>
    </main>
  );
}

function isOk(body: unknown): boolean {
  return typeof body === "object" && body !== null && "ok" in body && body.ok === true;
}
