# ADR 0001: Worker を Rust で続行する

## Status

Accepted

## Context

M1 スパイクで、Worker を Rust（workers-rs）にするか TypeScript（Hono）に退避するかを判断する。2026-10-05 にブランチ `m1-spike` で次を確認した。失敗はなく、すべて成功した。

1. fsrs 6.6.2 は `wasm32-unknown-unknown` でビルドでき、動作した。`GET /api/demo-schedule` は新規カードに Good を付け、間隔 2.3065 日を返した。
2. 前提として、getrandom 0.4 の `wasm_js` が必要だった。公式手順どおり、Worker の target dependency で feature を有効にし、`.cargo/config.toml` に `getrandom_backend="wasm_js"` の cfg を置いた。
3. workers-rs 0.8.7 の feature `d1` で D1 の往復が通った。`prepare` / `bind` / `run` / `first` を使う。ローカルは placeholder の database id と `wrangler d1 migrations apply --local` で動く。
4. worker-build 0.8.7 は nix dev shell 内の `cargo install` で再現できた（#14）。`wrangler.toml` の build command は crates.io から都度 install するため、バージョンはロックされない。これは既知の緩みである。
5. ts-rs 12.0.1 で Rust から TypeScript 型を生成できた。`cargo test` が `web/src/types/` に書き、生成物はコミットする。

## Decision

Worker は Rust（workers-rs）で続行する。TypeScript（Hono）への退避は不要。

## Consequences

- wasm 側の失敗は CI の `just ci`（wasm の clippy と build）が検出する。
- fsrs は rand と rayon を無条件に依存として引く。wasm32 では問題にならなかった。
- worker-build のバージョン固定は、必要になったら flake に固定して対応する。
- 2026-10-06: CI の Linux で cargo install 版が libssl を見つけられず失敗したため、nixpkgs の worker-build を flake で固定した。
