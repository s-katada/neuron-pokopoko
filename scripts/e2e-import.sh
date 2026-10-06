#!/usr/bin/env bash
# 一時 D1 で復習し、別の D1 に JSONL から戻して export と FSRS 列が一致することを確かめる。
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

answer_rating() {
  local rating=$1 body key code
  body=$(curl -sS "$ENDPOINT/api/review/next")
  key=$(jq -r '.card.stable_key // empty' <<<"$body")
  if [[ -z "$key" ]]; then
    echo "next が空: $body" >&2
    exit 1
  fi
  jq -n --arg key "$key" --arg rating "$rating" '{stable_key:$key, rating:$rating}' >"$TMP/answer-req.json"
  code=$(curl -sS -o "$TMP/answer.json" -w '%{http_code}' -X POST \
    -H 'content-type: application/json' \
    --data-binary @"$TMP/answer-req.json" \
    "$ENDPOINT/api/review/answer")
  if [[ "$code" != 200 ]]; then
    echo "answer: 期待 200 実際 $code $(<"$TMP/answer.json")" >&2
    exit 1
  fi
  echo "$key"
}

first=$(answer_rating good)
echo "first: $first"
second=$(answer_rating good)
echo "second: $second"
d1_exec "UPDATE cards SET due_at = 0 WHERE stable_key = '$first'"
again=$(answer_rating again)
echo "again: $again"
if [[ "$again" != "$first" ]]; then
  echo "again: 期待 $first 実際 $again" >&2
  exit 1
fi

"$POKO" export "$TMP/a.jsonl" --endpoint "$ENDPOINT" --page-size 2 >/dev/null
fsrs_sql="SELECT stable_key, fsrs_state, stability, difficulty, due_at, last_reviewed_at, reps, lapses FROM cards WHERE last_reviewed_at IS NOT NULL ORDER BY stable_key"
before=$(d1_json "$fsrs_sql" | jq -c '[.[0].results[] | {stable_key, fsrs_state, stability, difficulty, due_at, last_reviewed_at, reps, lapses}]')
echo "before: $before"

e2e_stop_worker
STATE="$TMP/state2"
e2e_start_worker

out=$("$POKO" sync "$POLICY" --endpoint "$ENDPOINT")
echo "resync: $out"
out=$("$POKO" import "$TMP/a.jsonl" --endpoint "$ENDPOINT")
echo "import: $out"
"$POKO" export "$TMP/b.jsonl" --endpoint "$ENDPOINT" --page-size 2 >/dev/null
if ! cmp -s "$TMP/a.jsonl" "$TMP/b.jsonl"; then
  echo "JSONL が一致しない" >&2
  diff -u "$TMP/a.jsonl" "$TMP/b.jsonl" >&2 || true
  exit 1
fi
echo "jsonl: match"
after=$(d1_json "$fsrs_sql" | jq -c '[.[0].results[] | {stable_key, fsrs_state, stability, difficulty, due_at, last_reviewed_at, reps, lapses}]')
expect_eq "fsrs" "$before" "$after"

out=$("$POKO" import "$TMP/a.jsonl" --endpoint "$ENDPOINT")
echo "reimport: $out"
case "$out" in
  "imported 0,"*) ;;
  *)
    echo "reimport: 期待 imported 0 実際 $out" >&2
    exit 1
    ;;
esac

echo "E2E IMPORT OK"
