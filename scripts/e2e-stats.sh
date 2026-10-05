#!/usr/bin/env bash
# 一時ディレクトリの D1 で、統計 API を通す。開発用の .wrangler/state は触らない。
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

is_unit() {
  jq -e -n --argjson n "$1" '$n >= 0 and $n <= 1' >/dev/null
}

POLICY="$ROOT/crates/core/tests/fixtures/vault-policy"
out=$("$POKO" sync "$POLICY" --endpoint "$ENDPOINT")
echo "step1: $out"
if [[ "$out" != *"6 notes"* || "$out" != *"(13 cards)"* ]]; then
  fail "1 sync" "6 notes, 13 cards" "$out"
fi

tree=$(curl -sS "$ENDPOINT/api/tree")
majors=$(jq -r '.majors | length' <<<"$tree")
cards=$(jq -r '[.majors[].card_count] | add' <<<"$tree")
overall=$(jq -r '.overall' <<<"$tree")
if [[ "$majors" != 2 || "$cards" != 13 || "$overall" != "null" ]]; then
  fail "2 GET /api/tree" "大分類 2、card_count 合計 13、overall null" "大分類 $majors、card_count $cards、overall $overall"
fi
echo "step2: majors=$majors cards=$cards overall=null"

first_key=""
first_level=""
first_question=""
for i in 1 2 3; do
  next=$(curl -sS "$ENDPOINT/api/review/next")
  key=$(jq -r '.card.stable_key // empty' <<<"$next")
  if [[ -z "$key" ]]; then
    fail "3 GET /api/review/next ($i)" "カードが出る" "$next"
  fi
  jq -n --arg key "$key" '{stable_key:$key, rating:"good"}' >"$TMP/answer-req.json"
  code=$(curl -sS -o "$TMP/answer.json" -w '%{http_code}' -X POST \
    -H 'content-type: application/json' \
    --data-binary @"$TMP/answer-req.json" \
    "$ENDPOINT/api/review/answer")
  if [[ "$code" != 200 ]]; then
    fail "3 POST /api/review/answer ($i)" "200" "$code $(<"$TMP/answer.json")"
  fi
  echo "step3.$i: $key"
  if [[ "$i" == 1 ]]; then
    first_key=$key
    first_level=$(jq -r '.card.level' <<<"$next")
    first_question=$(jq -r '.card.question' <<<"$next")
  fi
done

tree=$(curl -sS "$ENDPOINT/api/tree")
overall=$(jq -c '.overall' <<<"$tree")
if ! is_unit "$overall"; then
  fail "4 GET /api/tree overall" "0 以上 1 以下の数" "$overall"
fi
majors_retention=$(jq -c '[.majors[] | {major, retention}]' <<<"$tree")
if ! jq -e 'all(.[]; .retention == null or (.retention | type == "number" and . >= 0 and . <= 1))' <<<"$majors_retention" >/dev/null; then
  fail "4 大分類の retention" "null または 0〜1" "$majors_retention"
fi
echo "step4: overall=$overall majors=$majors_retention"

note_id=${first_key%%/*}
detail=$(curl -sS "$ENDPOINT/api/notes/$note_id")
card=$(jq -c --arg level "$first_level" --arg question "$first_question" \
  '[.cards[] | select(.level == $level and .question == $question)] | .[0] // empty' <<<"$detail")
if [[ -z "$card" ]]; then
  fail "5 GET /api/notes/$note_id" "回答したカードが含まれる" "$detail"
fi
due=$(jq -r '.due_at' <<<"$card")
retention=$(jq -c '.retention' <<<"$card")
now=$(date +%s)
if [[ ! "$due" =~ ^[0-9]+$ ]] || (( due <= now )) || ! is_unit "$retention"; then
  fail "5 回答したカード" "due_at が $now より未来、retention が 0〜1" "due_at=$due retention=$retention"
fi
echo "step5: note=$note_id due_at=$due retention=$retention"

daily=$(curl -sS "$ENDPOINT/api/stats/daily")
length=$(jq -r 'length' <<<"$daily")
today=$(jq -r '.[-1].count' <<<"$daily")
rest=$(jq -r '[.[0:-1][].count] | add // 0' <<<"$daily")
if [[ "$length" != 30 || "$today" != 3 || "$rest" != 0 ]]; then
  fail "6 GET /api/stats/daily" "長さ 30、今日 3、それ以外の合計 0" "長さ $length、今日 $today、それ以外 $rest"
fi
echo "step6: length=$length today=$today rest=$rest"

week=$(curl -sS "$ENDPOINT/api/stats/daily?days=7")
week_len=$(jq -r 'if type == "array" then length else . end' <<<"$week")
if [[ "$week_len" != 7 ]]; then
  fail "7 GET /api/stats/daily?days=7" "長さ 7" "$week"
fi

for bad_days in 0 91; do
  code=$(curl -sS -o "$TMP/days.json" -w '%{http_code}' "$ENDPOINT/api/stats/daily?days=$bad_days")
  if [[ "$code" != 400 ]]; then
    fail "7 GET /api/stats/daily?days=$bad_days" "400" "$code $(<"$TMP/days.json")"
  fi
done

code=$(curl -sS -o "$TMP/missing.json" -w '%{http_code}' "$ENDPOINT/api/notes/nope")
if [[ "$code" != 404 ]]; then
  fail "7 GET /api/notes/nope" "404" "$code $(<"$TMP/missing.json")"
fi
echo "step7: days=7 length=7, days=0/91 400, nope 404"

echo "E2E STATS OK"
