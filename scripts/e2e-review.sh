#!/usr/bin/env bash
# 一時ディレクトリの D1 で、fixture vault の出題を通す。開発用の .wrangler/state は触らない。
set -euo pipefail

ENDPOINT="http://localhost:8788"
# shellcheck source=e2e-lib.sh
source "$(dirname "$0")/e2e-lib.sh"
e2e_init
e2e_prepare

out=$("$POKO" sync "$VAULT" --endpoint "$ENDPOINT")
echo "step2: $out"
require_substr "step2" "$out" "(4 cards)"

next=$(curl -sS "$ENDPOINT/api/review/next")
key=$(jq -r '.card.stable_key // empty' <<<"$next")
if [[ "$key" != gain/beginner/* ]]; then
  echo "step3 stable_key: 期待 gain/beginner/ で始まる 実際 ${key:-$next}" >&2
  exit 1
fi
echo "step3: $key"

jq -n --arg key "$key" '{stable_key:$key, rating:"good"}' >"$TMP/answer-req.json"
code=$(curl -sS -o "$TMP/answer.json" -w '%{http_code}' -X POST \
  -H 'content-type: application/json' \
  --data-binary @"$TMP/answer-req.json" \
  "$ENDPOINT/api/review/answer")
if [[ "$code" != 200 ]]; then
  echo "step4 status: 期待 200 実際 $code $(<"$TMP/answer.json")" >&2
  exit 1
fi
due=$(jq -r '.due_at' "$TMP/answer.json")
now=$(date +%s)
if (( due <= now )); then
  echo "step4 due_at: 期待 $now より未来 実際 $due" >&2
  exit 1
fi
echo "step4 due_at=$due"

expect_eq "step5 reviews" 1 "$(d1_n "SELECT count(*) AS n FROM reviews")"
expect_eq "step5 rating" good "$(d1_n "SELECT rating AS n FROM reviews")"

done_body=$(curl -sS "$ENDPOINT/api/review/next")
if ! jq -e '.card == null' <<<"$done_body" >/dev/null; then
  echo "step6: 期待 {\"card\": null} 実際 $done_body" >&2
  exit 1
fi
echo "step6: $done_body"

echo "E2E REVIEW OK"
