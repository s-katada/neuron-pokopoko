#!/usr/bin/env bash
# 一時ディレクトリの D1 で、fixture vault の同期を通す。開発用の .wrangler/state は触らない。
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
TMP=$(mktemp -d)
STATE="$TMP/state"
VAULT="$TMP/vault"
ENDPOINT="http://127.0.0.1:8788"
PID=""

cleanup() {
  if [[ -n "$PID" ]]; then
    kill "$PID" 2>/dev/null || true
    pkill -P "$PID" 2>/dev/null || true
  fi
  if command -v lsof >/dev/null 2>&1; then
    local listeners
    listeners=$(lsof -nP -iTCP:8788 -sTCP:LISTEN -t 2>/dev/null || true)
    if [[ -n "$listeners" ]]; then
      # shellcheck disable=SC2086
      kill $listeners 2>/dev/null || true
    fi
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT

command -v jq >/dev/null || {
  echo "jq が PATH に無い" >&2
  exit 1
}

d1_json() {
  (
    cd "$ROOT/crates/worker"
    wrangler d1 execute neuron-pokopoko --local --persist-to "$STATE" --json --command "$1"
  )
}

d1_n() {
  d1_json "$1" | jq -r '.[0].results[0].n'
}

d1_exec() {
  (
    cd "$ROOT/crates/worker"
    wrangler d1 execute neuron-pokopoko --local --persist-to "$STATE" --command "$1"
  )
}

expect_eq() {
  local label=$1 expected=$2 actual=$3
  if [[ "$actual" != "$expected" ]]; then
    echo "$label: 期待 $expected 実際 $actual" >&2
    exit 1
  fi
  echo "$label=$actual"
}

require_substr() {
  local label=$1 haystack=$2 needle=$3
  if [[ "$haystack" != *"$needle"* ]]; then
    echo "$label: '$needle' が無い: $haystack" >&2
    exit 1
  fi
}

mkdir -p "$VAULT"
cp -R "$ROOT/crates/core/tests/fixtures/vault/." "$VAULT/"
cargo build -q -p poko
POKO="$ROOT/target/debug/poko"

(
  cd "$ROOT/crates/worker"
  wrangler d1 migrations apply neuron-pokopoko --local --persist-to "$STATE"
)

(
  cd "$ROOT/crates/worker"
  wrangler dev --port 8788 --persist-to "$STATE"
) >"$TMP/wrangler.log" 2>&1 &
PID=$!

ready=0
for _ in $(seq 1 180); do
  if curl -sf "$ENDPOINT/api/health" >/dev/null; then
    ready=1
    break
  fi
  if ! kill -0 "$PID" 2>/dev/null; then
    echo "wrangler dev が終了した" >&2
    tail -n 40 "$TMP/wrangler.log" >&2
    exit 1
  fi
  sleep 1
done
if [[ "$ready" != 1 ]]; then
  echo "health が 180 秒以内に返らなかった" >&2
  tail -n 40 "$TMP/wrangler.log" >&2
  exit 1
fi

run_sync() {
  "$POKO" sync "$VAULT" --endpoint "$ENDPOINT"
}

out=$(run_sync)
echo "step4: $out"
expect_eq "step4 cards" 4 "$(d1_n "SELECT count(*) AS n FROM cards")"

out=$(run_sync)
echo "step5: $out"
require_substr "step5" "$out" "unchanged 1"
expect_eq "step5 cards" 4 "$(d1_n "SELECT count(*) AS n FROM cards")"

d1_exec "UPDATE cards SET stability = 3.5 WHERE question = 'ゲインとは？'"
d1_exec "INSERT INTO reviews (card_key, rating, reviewed_at, interval_days, stability, difficulty) SELECT stable_key, 'good', 1, 1.0, 2.5, 5.0 FROM cards WHERE question = 'ゲインとは？'"

gain="$VAULT/learning/image-processing/camera/exposure/gain.md"
old='ゲインとは、撮像素子の出力信号を増幅する倍率である。'
new="${old}E2E"
content=$(<"$gain")
if [[ "$content" != *"$old"* ]]; then
  echo "step6: 原文が gain.md に無い" >&2
  exit 1
fi
printf '%s' "${content/"$old"/"$new"}" >"$gain"

out=$(run_sync)
echo "step6: $out"
expect_eq "step6 reviews" 1 "$(d1_n "SELECT count(*) AS n FROM reviews")"
stab=$(d1_n "SELECT stability AS n FROM cards WHERE question = 'ゲインとは？'")
if ! printf '%s\n' "$stab" | jq -e '. == 3.5' >/dev/null; then
  echo "step6 stability: 期待 3.5 実際 $stab" >&2
  exit 1
fi
echo "step6 stability=$stab"
expect_eq "step6 answer" "$new" "$(d1_n "SELECT answer AS n FROM cards WHERE question = 'ゲインとは？'")"

rm "$gain"
out=$(run_sync)
echo "step7: $out"
require_substr "step7" "$out" "deleted 1"
expect_eq "step7 active" 0 "$(d1_n "SELECT count(*) AS n FROM cards WHERE retired_at IS NULL")"
expect_eq "step7 reviews" 1 "$(d1_n "SELECT count(*) AS n FROM reviews")"

echo "E2E OK"
