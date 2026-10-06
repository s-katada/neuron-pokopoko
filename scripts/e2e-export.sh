#!/usr/bin/env bash
# 一時 D1 で数件復習し、ページをまたいだ JSONL が reviews と一致することを確かめる。
set -euo pipefail

ENDPOINT="http://localhost:8788"
# shellcheck source=e2e-lib.sh
source "$(dirname "$0")/e2e-lib.sh"
e2e_init
e2e_prepare

POLICY="$ROOT/crates/core/tests/fixtures/vault-policy"
out=$("$POKO" sync "$POLICY" --endpoint "$ENDPOINT")
echo "sync: $out"
require_substr "sync" "$out" "(13 cards)"

answer_next() {
  local body key code
  body=$(curl -sS "$ENDPOINT/api/review/next")
  key=$(jq -r '.card.stable_key // empty' <<<"$body")
  if [[ -z "$key" ]]; then
    echo "next が空: $body" >&2
    exit 1
  fi
  jq -n --arg key "$key" '{stable_key:$key, rating:"good"}' >"$TMP/answer-req.json"
  code=$(curl -sS -o "$TMP/answer.json" -w '%{http_code}' -X POST \
    -H 'content-type: application/json' \
    --data-binary @"$TMP/answer-req.json" \
    "$ENDPOINT/api/review/answer")
  if [[ "$code" != 200 ]]; then
    echo "answer: 期待 200 実際 $code $(<"$TMP/answer.json")" >&2
    exit 1
  fi
  echo "answered $key"
}

answer_next
answer_next
answer_next

rows=$(d1_n "SELECT count(*) AS n FROM reviews")
out=$("$POKO" export "$TMP/reviews.jsonl" --endpoint "$ENDPOINT" --page-size 2)
echo "export: $out"
expect_eq "export" "exported $rows" "$out"
lines=$(grep -c . "$TMP/reviews.jsonl")
expect_eq "lines" "$rows" "$lines"
if grep -q '"id"' "$TMP/reviews.jsonl"; then
  echo "JSONL に id がある" >&2
  exit 1
fi

db=$(d1_json "SELECT card_key, reviewed_at FROM reviews ORDER BY id" \
  | jq -c '[.[0].results[] | [.card_key, (.reviewed_at | tonumber)]]')
got=$(jq -sc 'map([.card_key, (.reviewed_at | tonumber)])' "$TMP/reviews.jsonl")
expect_eq "order" "$db" "$got"

echo "E2E EXPORT OK"
