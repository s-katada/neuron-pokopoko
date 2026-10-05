#!/usr/bin/env bash
# 一時ディレクトリの D1 で、段の解禁・兄弟・交互・新規枠を通す。開発用の .wrangler/state は触らない。
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

future=$(($(date +%s) + 30 * 86400))
d1_exec "UPDATE cards SET fsrs_state = 'review', stability = 7.0, due_at = $future WHERE note_id = 'y3' AND level = 'beginner'"
d1_exec "UPDATE cards SET fsrs_state = 'review', stability = 6.99, due_at = $future WHERE note_id = 'x3' AND level = 'beginner'"
expect_eq "y3 unlock" 1 "$(d1_n "SELECT count(*) AS n FROM cards WHERE note_id = 'y3' AND level = 'beginner' AND stability = 7.0 AND fsrs_state = 'review'")"
expect_eq "x3 lock" 1 "$(d1_n "SELECT count(*) AS n FROM cards WHERE note_id = 'x3' AND level = 'beginner' AND stability = 6.99 AND fsrs_state = 'review'")"

answer_good() {
  local key=$1
  jq -n --arg key "$key" '{stable_key:$key, rating:"good"}' >"$TMP/answer-req.json"
  local code
  code=$(curl -sS -o "$TMP/answer.json" -w '%{http_code}' -X POST \
    -H 'content-type: application/json' \
    --data-binary @"$TMP/answer-req.json" \
    "$ENDPOINT/api/review/answer")
  if [[ "$code" != 200 ]]; then
    echo "answer: 期待 200 実際 $code $(<"$TMP/answer.json")" >&2
    exit 1
  fi
}

expect_prefix() {
  local step=$1 prefix=$2
  local body key
  body=$(curl -sS "$ENDPOINT/api/review/next")
  key=$(jq -r '.card.stable_key // empty' <<<"$body")
  if [[ "$step" == "c" && "$key" == x1/beginner/* ]]; then
    echo "c: 期待 x2/beginner/ 実際 $key（x1 の兄弟が出た）" >&2
    exit 1
  fi
  if [[ "$key" != "$prefix"* ]]; then
    echo "$step: 期待 ${prefix} で始まる 実際 ${key:-$body}" >&2
    exit 1
  fi
  echo "$step: $key"
  answer_good "$key"
}

expect_prefix a "x1/beginner/"
expect_prefix b "y1/beginner/"
expect_prefix c "x2/beginner/"
expect_prefix d "y2/beginner/"
expect_prefix e "y3/intermediate/"

done_body=$(curl -sS "$ENDPOINT/api/review/next")
if ! jq -e '.card == null' <<<"$done_body" >/dev/null; then
  echo "f: 期待 card null 実際 $done_body" >&2
  exit 1
fi
echo "f: null"

echo "E2E POLICY OK"
