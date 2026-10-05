#!/usr/bin/env bash
# 一時ディレクトリの D1 で、上級と統合の解禁を通す。開発用の .wrangler/state は触らない。
set -euo pipefail

ENDPOINT="http://localhost:8788"
# shellcheck source=e2e-lib.sh
source "$(dirname "$0")/e2e-lib.sh"
e2e_init
e2e_prepare

fail() {
  echo "ステップ: $1" >&2
  echo "期待: $2" >&2
  echo "実際: $3" >&2
  exit 1
}

VAULT_INTEGRATION="$ROOT/crates/core/tests/fixtures/vault-integration"
out=$("$POKO" sync "$VAULT_INTEGRATION" --endpoint "$ENDPOINT")
echo "step1: $out"
if [[ "$out" != *"3 notes"* || "$out" != *"(8 cards)"* ]]; then
  fail "1 sync" "3 notes, 8 cards" "$out"
fi

d1_exec "UPDATE cards SET fsrs_state = 'review', stability = 7.0, due_at = strftime('%s','now') + 86400 * 400 WHERE level = 'beginner'; UPDATE cards SET fsrs_state = 'review', stability = 21.0, due_at = strftime('%s','now') + 86400 * 400 WHERE note_id IN ('i1', 'i2') AND level = 'intermediate'; UPDATE cards SET fsrs_state = 'review', stability = 20.99, due_at = strftime('%s','now') + 86400 * 400 WHERE note_id = 'i3' AND level = 'intermediate';"
beginners=$(d1_n "SELECT count(*) AS n FROM cards WHERE level = 'beginner' AND fsrs_state = 'review' AND stability = 7.0 AND due_at > strftime('%s','now')")
unlocked_mid=$(d1_n "SELECT count(*) AS n FROM cards WHERE note_id IN ('i1', 'i2') AND level = 'intermediate' AND fsrs_state = 'review' AND stability = 21.0 AND due_at > strftime('%s','now')")
locked_mid=$(d1_n "SELECT count(*) AS n FROM cards WHERE note_id = 'i3' AND level = 'intermediate' AND fsrs_state = 'review' AND stability = 20.99 AND due_at > strftime('%s','now')")
reviews_before=$(d1_n "SELECT count(*) AS n FROM reviews")
if [[ "$beginners" != 3 || "$unlocked_mid" != 2 || "$locked_mid" != 1 || "$reviews_before" != 0 ]]; then
  fail "2 解禁前の状態" "初級 3、i1・i2 の中級 2、i3 の中級 1、reviews 0" "初級 $beginners、i1・i2 の中級 $unlocked_mid、i3 の中級 $locked_mid、reviews $reviews_before"
fi
echo "step2: parked, reviews=0"

answer_good() {
  local key=$1 response=$2
  jq -n --arg key "$key" --arg response "$response" '{stable_key:$key, rating:"good", response:$response}' >"$TMP/answer-req.json"
  local code
  code=$(curl -sS -o "$TMP/answer.json" -w '%{http_code}' -X POST \
    -H 'content-type: application/json' \
    --data-binary @"$TMP/answer-req.json" \
    "$ENDPOINT/api/review/answer")
  if [[ "$code" != 200 ]]; then
    fail "$3" "200" "$code $(<"$TMP/answer.json")"
  fi
}

next=$(curl -sS "$ENDPOINT/api/review/next")
key=$(jq -r '.card.stable_key // empty' <<<"$next")
rubric=$(jq -r '.card.rubric // empty' <<<"$next")
if [[ "$key" != i2/advanced/* || -z "$rubric" ]]; then
  fail "a GET next" "stable_key が i2/advanced/ で始まり rubric が空でない" "$next"
fi
echo "a: $key"
answer_good "$key" "最後に回す" "a POST answer"
saved=$(d1_n "SELECT count(*) AS n FROM reviews WHERE card_key = '$key' AND response = '最後に回す'")
if [[ "$saved" != 1 ]]; then
  fail "a reviews" "response=最後に回す の行が 1" "count=$saved"
fi

blocked=$(curl -sS "$ENDPOINT/api/review/next")
if ! jq -e '.card == null' <<<"$blocked" >/dev/null; then
  fail "b GET next" "card が null" "$blocked"
fi
echo "b: null"

d1_exec "UPDATE cards SET stability = 21.0 WHERE note_id = 'i3' AND level = 'intermediate';"
raised=$(d1_n "SELECT count(*) AS n FROM cards WHERE note_id = 'i3' AND level = 'intermediate' AND stability = 21.0")
if [[ "$raised" != 1 ]]; then
  fail "5 i3 の中級を 21.0" "1 件" "$raised 件"
fi

next=$(curl -sS "$ENDPOINT/api/review/next")
key=$(jq -r '.card.stable_key // empty' <<<"$next")
rubric=$(jq -r '.card.rubric // empty' <<<"$next")
titles=$(jq -c '.card.ref_titles' <<<"$next")
if [[ "$key" != i1/integration/* || "$titles" != '["ゲイン側","照明側"]' || -z "$rubric" ]]; then
  fail "c GET next" "i1/integration/ で始まり ref_titles=[ゲイン側, 照明側] かつ rubric が空でない" "$next"
fi
echo "c: $key"
answer_good "$key" "照明が先" "c POST answer"
saved=$(d1_n "SELECT count(*) AS n FROM reviews WHERE card_key = '$key' AND response = '照明が先'")
if [[ "$saved" != 1 ]]; then
  fail "c reviews" "response=照明が先 の行が 1" "count=$saved"
fi

done_body=$(curl -sS "$ENDPOINT/api/review/next")
if ! jq -e '.card == null' <<<"$done_body" >/dev/null; then
  fail "d GET next" "card が null" "$done_body"
fi
echo "d: null"

echo "E2E INTEGRATION OK"
