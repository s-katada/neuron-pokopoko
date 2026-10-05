# ニューロンポコポコ

学んだことを、忘れた頃に出題してくれる個人用の復習アプリ。忘却への復讐。

```
Obsidian vault (git)
   │ poko (CLI, Rust): lint / 差分 sync
   ▼
Cloudflare Worker (Rust + D1) ◄──► SPA (React)
```

- **BE / CLI**: Rust(workers-rs + axum + D1)。出題間隔は [FSRS](https://github.com/open-spaced-repetition/fsrs-rs)
- **FE**: React 19 + Vite+ + Tailwind CSS v4
- **認証**: Cloudflare Access(アプリ側に認証コードなし)。すべて無料プランで動かす

## 開発

前提は nix + direnv。`direnv allow` で rust / node / pnpm / wrangler / just が揃う。

初回だけ、フロントの依存を入れる。

```bash
cd web && pnpm install
```

ローカル開発はターミナルを 2 つ開く。

```bash
# ① API :8787
cd crates/worker && wrangler dev

# ② SPA :5173。/api は 8787 へ proxy
cd web && pnpm dev
```

検証は次の 2 つ。`just ci` は fmt-check、clippy、wasm clippy、test、wasm build。

```bash
just ci
cd web && pnpm check
```

同期の通し確認は `just e2e-sync`（`just ci` には入らない）。
出題の通し確認は `just e2e-review`（`just ci` には入らない）。

```
crates/core
crates/cli
crates/worker
web/
```

進め方は [issues](../../issues) を参照(実装: cursor-agent / レビュー: Claude / マージ: 人間)。
