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

## 開発環境

```bash
direnv allow   # nix flake が rust / node / pnpm / wrangler を固定
```

進め方は [issues](../../issues) を参照(実装: cursor-agent / レビュー: Claude / マージ: 人間)。
