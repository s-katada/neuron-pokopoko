#!/usr/bin/env bash
# 一時ディレクトリの D1 で、中級の自由記述を通す。開発用の .wrangler/state は触らない。
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

out=$("$POKO" sync "$VAULT" --endpoint "$ENDPOINT")
echo "step1: $out"
if [[ "$out" != *"(4 cards)"* ]]; then
  fail "1 sync" "(4 cards) を含む" "$out"
fi

d1_exec "UPDATE cards SET fsrs_state = 'review', stability = 7.0, due_at = strftime('%s','now') + 86400 * 400 WHERE note_id = 'gain' AND level = 'beginner';"
parked=$(d1_n "SELECT count(*) AS n FROM cards WHERE note_id = 'gain' AND level = 'beginner' AND fsrs_state = 'review' AND stability = 7.0 AND due_at > strftime('%s','now')")
reviews_before=$(d1_n "SELECT count(*) AS n FROM reviews")
if [[ "$parked" != 1 || "$reviews_before" != 0 ]]; then
  fail "2 初級を未来の review にする" "対象 1 件、reviews 0 件" "対象 $parked 件、reviews $reviews_before 件"
fi
echo "step2: beginner parked, reviews=0"

next=$(curl -sS "$ENDPOINT/api/review/next")
key=$(jq -r '.card.stable_key // empty' <<<"$next")
level=$(jq -r '.card.level // empty' <<<"$next")
rubric=$(jq -r '.card.rubric // empty' <<<"$next")
if [[ "$level" != intermediate || -z "$rubric" || -z "$key" ]]; then
  fail "3 GET next" "level=intermediate かつ rubric が空でない" "$next"
fi
echo "step3: $key rubric=${#rubric}chars"

jq -n --arg key "$key" '{stable_key:$key, rating:"good", response:"受光量は増えない"}' >"$TMP/answer-req.json"
code=$(curl -sS -o "$TMP/answer.json" -w '%{http_code}' -X POST \
  -H 'content-type: application/json' \
  --data-binary @"$TMP/answer-req.json" \
  "$ENDPOINT/api/review/answer")
if [[ "$code" != 200 ]]; then
  fail "4 POST answer" "200" "$code $(<"$TMP/answer.json")"
fi
echo "step4: 200"

review_n=$(d1_n "SELECT count(*) AS n FROM reviews WHERE card_key = '$key'")
review_response=$(d1_n "SELECT response AS n FROM reviews WHERE card_key = '$key'")
card_state=$(d1_n "SELECT fsrs_state AS n FROM cards WHERE stable_key = '$key'")
if [[ "$review_n" != 1 || "$review_response" != "受光量は増えない" || "$card_state" != review ]]; then
  fail "5 保存" "reviews 1 行、response=受光量は増えない、fsrs_state=review" "reviews $review_n 行、response=$review_response、fsrs_state=$card_state"
fi
echo "step5: response saved, state=review"

long=$(jq -nr '[range(2001) | "あ"] | join("")')
jq -n --arg key "$key" --arg response "$long" '{stable_key:$key, rating:"good", response:$response}' >"$TMP/long-req.json"
long_code=$(curl -sS -o "$TMP/long.json" -w '%{http_code}' -X POST \
  -H 'content-type: application/json' \
  --data-binary @"$TMP/long-req.json" \
  "$ENDPOINT/api/review/answer")
if [[ "$long_code" != 400 ]]; then
  fail "6 2001 文字" "400" "$long_code $(<"$TMP/long.json")"
fi
echo "step6: 400"

echo "E2E INTERMEDIATE OK"
