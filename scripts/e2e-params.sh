#!/usr/bin/env bash
# 一時 D1 で、保存した FSRS パラメータが新規カードの good 間隔を変えることを確かめる。
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

# fsrs 6.6.2 の DEFAULT_PARAMETERS (crates.io fsrs の inference.rs)
default='[0.212,1.2931,2.3065,8.2956,6.4133,0.8334,3.0194,0.001,1.8722,0.1666,0.796,1.4835,0.0614,0.2629,1.6483,0.6014,1.8729,0.5425,0.0912,0.0658,0.1542]'
params=$(jq -c '.[2] = 10' <<<"$default")

answer_good() {
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
  echo "$key"
}

first=$(answer_good)
interval1=$(jq -r '.interval_days' "$TMP/answer.json")
echo "first: $first interval=$interval1"

jq -n --argjson params "$params" '{params:$params, review_count:4}' >"$TMP/put.json"
code=$(curl -sS -o "$TMP/put-out.json" -w '%{http_code}' -X PUT \
  -H 'content-type: application/json' \
  --data-binary @"$TMP/put.json" \
  "$ENDPOINT/api/params")
if [[ "$code" != 200 ]]; then
  echo "put: 期待 200 実際 $code $(<"$TMP/put-out.json")" >&2
  exit 1
fi
got=$(curl -sS "$ENDPOINT/api/params")
got_params=$(jq -c '.params' <<<"$got")
if ! jq -e -n --argjson want "$params" --argjson got "$got_params" '$want == $got' >/dev/null; then
  echo "get: 期待 $params 実際 $got_params" >&2
  exit 1
fi
echo "params: match"

second=$(answer_good)
interval2=$(jq -r '.interval_days' "$TMP/answer.json")
echo "second: $second interval=$interval2"
if [[ "$second" == "$first" ]]; then
  echo "second: 別のカードが欲しい 実際 $second" >&2
  exit 1
fi
if ! jq -e -n --argjson a "$interval1" --argjson b "$interval2" '$a != $b' >/dev/null; then
  echo "interval: 期待 不一致 実際 $interval1 と $interval2" >&2
  exit 1
fi

short=$(jq -c '.[0:20]' <<<"$params")
jq -n --argjson params "$short" '{params:$params, review_count:4}' >"$TMP/short.json"
code=$(curl -sS -o "$TMP/short-out.json" -w '%{http_code}' -X PUT \
  -H 'content-type: application/json' \
  --data-binary @"$TMP/short.json" \
  "$ENDPOINT/api/params")
if [[ "$code" != 400 ]]; then
  echo "short: 期待 400 実際 $code $(<"$TMP/short-out.json")" >&2
  exit 1
fi
got=$(curl -sS "$ENDPOINT/api/params")
got_params=$(jq -c '.params' <<<"$got")
if ! jq -e -n --argjson want "$params" --argjson got "$got_params" '$want == $got' >/dev/null; then
  echo "get after 400: 期待 $params 実際 $got_params" >&2
  exit 1
fi
echo "short: rejected"

echo "E2E PARAMS OK"
