# ニューロンポコポコの開発タスク。`just <task>` で実行する

# タスク一覧
default:
    @just --list

# コードを整形する
fmt:
    cargo fmt --all

# 整形されているか確認する
fmt-check:
    cargo fmt --all --check

# native の clippy。警告はエラー
lint:
    cargo clippy --all-targets -- -D warnings

# worker を wasm32 で clippy。警告はエラー
lint-wasm:
    cargo clippy -p poko-worker --target wasm32-unknown-unknown -- -D warnings

# テストを実行する
test:
    cargo test

# worker を wasm32 でビルドする
build-wasm:
    cargo build -p poko-worker --target wasm32-unknown-unknown

# CI と同じ検証
ci: fmt-check lint lint-wasm test build-wasm

# SVG から PWA アイコンを生成する
icons:
    bash scripts/gen-icons.sh

# 一時 D1 で fixture vault の同期を通す。ci には入れない
e2e-sync:
    bash scripts/e2e-sync.sh

# 一時 D1 で fixture vault の出題を通す。ci には入れない
e2e-review:
    bash scripts/e2e-review.sh

# 一時 D1 で出題ポリシーを通す。ci には入れない
e2e-policy:
    bash scripts/e2e-policy.sh

# 一時 D1 で中級の自由記述を通す。ci には入れない
e2e-intermediate:
    bash scripts/e2e-intermediate.sh

# 一時 D1 で上級と統合の解禁を通す。ci には入れない
e2e-integration:
    bash scripts/e2e-integration.sh

# 一時 D1 で統計 API を通す。ci には入れない
e2e-stats:
    bash scripts/e2e-stats.sh
