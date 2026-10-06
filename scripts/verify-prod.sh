#!/usr/bin/env bash
# 本番が Access で守られ、サービストークンで health と manifest に届くか確かめる。
# データを書き換える操作はしない。secret は表示しない。
set -euo pipefail

usage() {
  echo "使い方: scripts/verify-prod.sh [URL]" >&2
  echo "URL を省略すると POKO_ENDPOINT を使う" >&2
  exit 1
}

if [[ $# -gt 1 ]]; then
  usage
fi
if [[ $# -eq 1 ]]; then
  base=$1
elif [[ -n "${POKO_ENDPOINT:-}" ]]; then
  base=$POKO_ENDPOINT
else
  usage
fi
base=${base%/}

pass() {
  echo "$1 PASS"
}

fail() {
  echo "$1 FAIL: $2"
  exit 1
}

status_of() {
  local output
  if ! output=$(curl -sS --max-redirs 0 -o "$2" -w '%{http_code}' "${@:3}" "$1"); then
    echo "000"
    return 0
  fi
  echo "$output"
}

body=$(mktemp)
trap 'rm -f "$body"' EXIT

code=$(status_of "$base/api/health" "$body")
if [[ "$code" =~ ^2 ]]; then
  fail 1 "Access で保護されていない"
elif [[ "$code" =~ ^3 || "$code" == "401" || "$code" == "403" ]]; then
  pass 1
else
  fail 1 "ステータス $code"
fi

missing=0
if [[ -z "${POKO_ACCESS_CLIENT_ID:-}" ]]; then
  echo "POKO_ACCESS_CLIENT_ID が無い" >&2
  missing=1
fi
if [[ -z "${POKO_ACCESS_CLIENT_SECRET:-}" ]]; then
  echo "POKO_ACCESS_CLIENT_SECRET が無い" >&2
  missing=1
fi
if [[ "$missing" == 1 ]]; then
  exit 1
fi

code=$(status_of "$base/api/health" "$body" \
  -H "CF-Access-Client-Id: ${POKO_ACCESS_CLIENT_ID}" \
  -H "CF-Access-Client-Secret: ${POKO_ACCESS_CLIENT_SECRET}")
if [[ "$code" == "200" ]] && jq -e '.ok == true' "$body" >/dev/null; then
  pass 2
else
  fail 2 "health が 200 の {\"ok\":true} ではない"
fi

code=$(status_of "$base/api/sync/manifest" "$body" \
  -H "CF-Access-Client-Id: ${POKO_ACCESS_CLIENT_ID}" \
  -H "CF-Access-Client-Secret: ${POKO_ACCESS_CLIENT_SECRET}")
if [[ "$code" == "200" ]] && jq -e 'type == "array"' "$body" >/dev/null; then
  pass 3
else
  fail 3 "manifest が 200 の JSON 配列ではない"
fi

echo "VERIFY PROD OK"
