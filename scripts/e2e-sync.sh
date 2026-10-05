#!/usr/bin/env bash
# 一時ディレクトリの D1 で、fixture vault の同期を通す。開発用の .wrangler/state は触らない。
set -euo pipefail

ENDPOINT="http://127.0.0.1:8788"
# shellcheck source=e2e-lib.sh
source "$(dirname "$0")/e2e-lib.sh"
e2e_init
e2e_prepare

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
