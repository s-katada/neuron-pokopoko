# ニューロンポコポコ

学びメモ(Obsidian vault)を FSRS で復習する個人用アプリ。

構成: `crates/core`(共有ロジック・純 Rust)/ `crates/cli`(poko)/ `crates/worker`(workers-rs + axum + D1)/ `web/`(React SPA)/ `migrations/`(D1)/ `docs/spec/`(ノート仕様)。

## コマンド

- すべて nix dev shell 経由(direnv、または `nix develop --command <cmd>`)。グローバルのツールに依存しない
- `just ci` = fmt-check + clippy(-D warnings)+ test。CI と同一(M0 で導入)
- web/ は pnpm。リポジトリ直下に package.json を置かない

## 進め方(厳守)

- 1 sub-issue = 1 コミット。テストは同じコミットに同梱し、各コミットで `just ci` が通る
- コミットは日本語。件名末尾に `(#<sub-issue番号>)`、本文に why を 1〜2 文
- 1 コミットの目安は数十〜150 行。超えそうなら実装前に相談する
- 指示された sub-issue の範囲だけ実装する。依存追加・リファクタ・スコープ拡大は提案のみ
- レビュー指摘の修正は該当コミットに amend(push 前のみ)。push と PR 作成は指示があってから
- PUBLIC リポジトリ。仕事由来の固有名詞(社名・案件名・社内 URL)を書かない

## 技術メモ(落とし穴)

- core は wasm32 と native の両方で動かす: 時刻は `now` を引数で受け取る。core 内で `SystemTime::now` や chrono の now を呼ばない
- 日付境界は JST(UTC+9)の 04:00。UTC 変換(`toISOString` 相当)で日付を作らない
- D1 Free: 1 呼び出し 50 クエリ / 1 クエリ 100 バインド。N+1 とバルク insert の分割に注意
- Worker Free: CPU 10ms/req。重いパース・差分計算は CLI 側でやる
- pnpm 11: overrides と allowBuilds は `pnpm-workspace.yaml` に書く(package.json の pnpm キーは読まれない)
- Vite+: import は `'vite-plus'` から。vitest は devDep に明示追加し Vite+ 同梱バージョンに固定。vitest 設定は vite.config.ts の test ブロック
- `wrangler dev` は assets を使う構成では dist が無いと起動しない(先に web をビルド)
